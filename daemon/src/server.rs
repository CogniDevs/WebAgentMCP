use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::prelude::*;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::cli::strip_unc_prefix;
use crate::config::{AppConfig, McpServerConfig};
use crate::i18n::{t, t_fmt, LocalesEmbed};
use crate::mcp::McpManager;
use crate::tools::{TerminalManager, ToolDispatcher};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub terminal: TerminalManager,
    pub mcp: McpManager,
    pub auth_token: String,
}

#[derive(Deserialize)]
pub struct RunToolRequest {
    pub name: String,
    pub payload: String,
}

#[derive(Serialize)]
pub struct RunToolResponse {
    pub output: String,
}

#[derive(Deserialize)]
pub struct SetWorkspaceRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct AddMcpRequest {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct RemoveMcpRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ToggleMcpRequest {
    pub name: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct SaveImageRequest {
    pub base64: String,
}

pub fn create_router(state: AppState) -> Router {
    let protected_routes = Router::new()
        .route("/api/run-tool", post(run_tool))
        .route("/api/save-image", post(save_image))
        .route("/api/workspace/pick", post(pick_workspace_folder))
        .route("/api/workspace/set", post(set_workspace_folder))
        .route("/api/mcp/add", post(add_mcp_server))
        .route("/api/mcp/remove", post(remove_mcp_server))
        .route("/api/mcp/toggle", post(toggle_mcp_server))
        .route("/api/shutdown", post(shutdown_server))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/locales", get(get_locales))
        .route("/api/system-prompt", get(get_system_prompt));

    Router::new()
        .merge(protected_routes)
        .merge(public_routes)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header_val) => {
            let token = header_val.trim_start_matches("Bearer ").trim();
            if token == state.auth_token {
                Ok(next.run(req).await)
            } else {
                let err_msg = t("unauthorized_api_error");
                let body = Json(json!({
                    "output": format!("Error: {}", err_msg),
                    "error": err_msg
                }));
                Err((StatusCode::UNAUTHORIZED, body).into_response())
            }
        }
        None => {
            let err_msg = t("unauthorized_api_error");
            let body = Json(json!({
                "output": format!("Error: {}", err_msg),
                "error": err_msg
            }));
            Err((StatusCode::UNAUTHORIZED, body).into_response())
        }
    }
}

async fn shutdown_server() -> impl IntoResponse {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let pid_file = crate::setup::get_pid_file();
        let _ = std::fs::remove_file(pid_file);
        std::process::exit(0);
    });
    Json(json!({ "status": "shutting_down" }))
}

async fn get_locales(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.lock().await;
    let lang = &cfg.language;
    let filename = format!("{}.json", lang);
    let fallback_name = "en.json";

    let file_data = LocalesEmbed::get(&filename)
        .or_else(|| LocalesEmbed::get(fallback_name));

    match file_data {
        Some(f) => {
            let val: Value = serde_json::from_slice(&f.data).unwrap_or(json!({}));
            Json(json!({ "language": lang, "strings": val }))
        }
        None => Json(json!({ "language": lang, "strings": {} })),
    }
}

async fn get_status(State(state): State<AppState>, req: Request) -> impl IntoResponse {
    let cfg = state.config.lock().await;
    let active_server_names = state.mcp.active_servers().await;
    let tools = state.mcp.list_tools().await;

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    let client_token = auth_header.map(|h| h.trim_start_matches("Bearer ").trim());
    let is_auth_valid = client_token == Some(&state.auth_token);

    let mut mcp_servers_info = Vec::new();
    for (name, srv_cfg) in &cfg.mcp_servers {
        let is_running = active_server_names.contains(name);
        mcp_servers_info.push(json!({
            "name": name,
            "command": srv_cfg.command,
            "args": srv_cfg.args,
            "enabled": srv_cfg.enabled,
            "status": if is_running { "connected" } else if !srv_cfg.enabled { "disabled" } else { "failed" }
        }));
    }

    Json(json!({
        "status": "online",
        "auth_valid": is_auth_valid,
        "workspace": strip_unc_prefix(&cfg.workspace_dir),
        "port": cfg.port,
        "language": cfg.language,
        "mcp_servers": mcp_servers_info,
        "mcp_tools_count": tools.len()
    }))
}

async fn pick_workspace_folder(State(state): State<AppState>) -> impl IntoResponse {
    let chosen = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new().pick_folder()
    })
    .await
    .unwrap_or(None);

    if let Some(path_buf) = chosen {
        let abs_str = strip_unc_prefix(&path_buf.to_string_lossy());
        let mut cfg = state.config.lock().await;
        if let Ok(updated) = cfg.set_workspace(&abs_str) {
            return Json(json!({ "success": true, "workspace": strip_unc_prefix(&updated) }));
        }
    }

    Json(json!({ "success": false }))
}

async fn set_workspace_folder(
    State(state): State<AppState>,
    Json(req): Json<SetWorkspaceRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.lock().await;
    let clean_path = strip_unc_prefix(&req.path);
    match cfg.set_workspace(&clean_path) {
        Ok(updated) => Json(json!({ "success": true, "workspace": strip_unc_prefix(&updated) })),
        Err(err) => Json(json!({ "success": false, "error": err })),
    }
}

async fn add_mcp_server(
    State(state): State<AppState>,
    Json(req): Json<AddMcpRequest>,
) -> impl IntoResponse {
    let srv_cfg = McpServerConfig {
        command: req.command,
        args: req.args,
        env: req.env,
        enabled: true,
    };

    let mut cfg = state.config.lock().await;
    let _ = cfg.add_mcp_server(req.name.clone(), srv_cfg.clone());

    let tools_count = state.mcp.add_and_start(req.name.clone(), srv_cfg).await.unwrap_or(0);

    Json(json!({ "success": true, "name": req.name, "tools_count": tools_count }))
}

async fn remove_mcp_server(
    State(state): State<AppState>,
    Json(req): Json<RemoveMcpRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.lock().await;
    let _ = cfg.remove_mcp_server(&req.name);
    state.mcp.remove(&req.name).await;

    Json(json!({ "success": true, "name": req.name }))
}

async fn toggle_mcp_server(
    State(state): State<AppState>,
    Json(req): Json<ToggleMcpRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.lock().await;
    let _ = cfg.toggle_mcp_server(&req.name, req.enabled);
    state.mcp.reload_all(&cfg).await;

    Json(json!({ "success": true, "name": req.name, "enabled": req.enabled }))
}

fn decode_base64_payload(data: &str) -> Result<(Vec<u8>, &'static str), String> {
    let (prefix, base64_raw) = if let Some(idx) = data.find(',') {
        (&data[..idx], &data[idx + 1..])
    } else {
        ("", data)
    };

    let ext = if prefix.contains("jpeg") || prefix.contains("jpg") {
        "jpg"
    } else if prefix.contains("webp") {
        "webp"
    } else if prefix.contains("svg") {
        "svg"
    } else {
        "png"
    };

    let clean = base64_raw.trim().replace(['\r', '\n', ' '], "");
    let decoded = BASE64_STANDARD
        .decode(clean.as_bytes())
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    Ok((decoded, ext))
}

async fn save_image(
    State(state): State<AppState>,
    Json(req): Json<SaveImageRequest>,
) -> impl IntoResponse {
    let (bytes, ext) = match decode_base64_payload(&req.base64) {
        Ok(res) => res,
        Err(err) => {
            return Json(json!({ "status": "error", "message": err }));
        }
    };

    let workspace = {
        let cfg = state.config.lock().await;
        cfg.workspace_dir.clone()
    };

    let creations_dir = Path::new(&workspace).join("agent-creations");
    if let Err(e) = fs::create_dir_all(&creations_dir) {
        return Json(json!({ "status": "error", "message": e.to_string() }));
    }

    let filename = format!("generated_{}.{}", &Uuid::new_v4().to_string()[..8], ext);
    let filepath = creations_dir.join(&filename);

    if let Err(e) = fs::write(&filepath, bytes) {
        return Json(json!({ "status": "error", "message": e.to_string() }));
    }

    let rel_path = format!("agent-creations/{}", filename);
    Json(json!({ "status": "ok", "path": rel_path }))
}

async fn run_tool(
    State(state): State<AppState>,
    Json(req): Json<RunToolRequest>,
) -> Result<Json<RunToolResponse>, (StatusCode, Json<Value>)> {
    let workspace = {
        let cfg = state.config.lock().await;
        cfg.workspace_dir.clone()
    };

    let clean_name = req.name
        .split(|c: char| c.is_whitespace() || c == '\'' || c == '"')
        .next()
        .unwrap_or(&req.name)
        .trim();

    let exec_msg = t_fmt("tool_executing", &[("name", clean_name)]);
    println!("{} {}", "⚡".bright_yellow().bold(), exec_msg.bold());

    if clean_name == "mcp_call" {
        let parsed_json: Value = match serde_json::from_str(&req.payload) {
            Ok(v) => v,
            Err(e) => {
                let err_text = format!("Invalid JSON for MCP: {}", e);
                eprintln!("  {} {}", "✗".red().bold(), t_fmt("tool_failed", &[("name", clean_name), ("error", &err_text)]));
                return Ok(Json(RunToolResponse { output: err_text }));
            }
        };

        let tool_name = match parsed_json
            .get("tool")
            .or_else(|| parsed_json.get("name"))
            .and_then(|v| v.as_str()) {
                Some(name) => name,
                None => {
                    let err_text = "Missing 'tool' or 'name' attribute in MCP payload".to_string();
                    eprintln!("  {} {}", "✗".red().bold(), t_fmt("tool_failed", &[("name", clean_name), ("error", &err_text)]));
                    return Ok(Json(RunToolResponse { output: err_text }));
                }
            };

        let arguments = parsed_json
            .get("arguments")
            .or_else(|| parsed_json.get("args"))
            .cloned()
            .unwrap_or(json!({}));

        match state.mcp.call_tool(tool_name, arguments).await {
            Ok(output) => {
                println!("  {} {}", "✓".green().bold(), t_fmt("tool_completed", &[("name", tool_name)]));
                Ok(Json(RunToolResponse { output }))
            }
            Err(err) => {
                eprintln!("  {} {}", "✗".red().bold(), t_fmt("tool_failed", &[("name", tool_name), ("error", &err)]));
                Ok(Json(RunToolResponse { output: format!("MCP Error: {}", err) }))
            }
        }
    } else {
        let mcp_tools = state.mcp.list_tools().await;
        if mcp_tools.iter().any(|t| t.name == clean_name) {
            let args: Value = serde_json::from_str(&req.payload).unwrap_or(json!({}));
            match state.mcp.call_tool(clean_name, args).await {
                Ok(output) => {
                    println!("  {} {}", "✓".green().bold(), t_fmt("tool_completed", &[("name", clean_name)]));
                    return Ok(Json(RunToolResponse { output }));
                }
                Err(err) => {
                    eprintln!("  {} {}", "✗".red().bold(), t_fmt("tool_failed", &[("name", clean_name), ("error", &err)]));
                    return Ok(Json(RunToolResponse { output: format!("MCP Error: {}", err) }));
                }
            }
        }

        match ToolDispatcher::dispatch(&workspace, &req.name, &req.payload, &state.terminal).await {
            Ok(output) => {
                println!("  {} {}", "✓".green().bold(), t_fmt("tool_completed", &[("name", clean_name)]));
                Ok(Json(RunToolResponse { output }))
            }
            Err(err) => {
                eprintln!("  {} {}", "✗".red().bold(), t_fmt("tool_failed", &[("name", clean_name), ("error", &err)]));
                Ok(Json(RunToolResponse { output: format!("Tool Error: {}", err) }))
            }
        }
    }
}

async fn get_system_prompt(State(state): State<AppState>) -> impl IntoResponse {
    let mcp_prompt = state.mcp.generate_tools_prompt().await;

    let base_prompt = [
        "You are WebAgentMCP, an autonomous software engineer connected to a local system execution bridge.",
        "You have direct execution access to the user's workspace and terminal via custom XML tool blocks.",
        "CRITICAL OPERATIONAL RULES:",
        "1. Output raw XML tool tags to perform actions. Every fact must originate from an actual tool result in this session.",
        "2. Stop your turn immediately after closing </tool>. Do not simulate or imagine results.",
        "3. AUTONOMOUS CONTINUATION: When a file or terminal output is large, the bridge returns the initial chunk with pagination metadata and an explicit follow-up call suggestion. If you need subsequent lines, invoke the recommended <tool='read_lines'> tag in your next turn. Never stop, apologize, or report failure due to chunked data.",
        "4. EXPLORE BEFORE YOU EDIT: Always use view_dir, search_code, or read_lines before patching files.",
        "5. IMAGE GENERATION: When you generate an image/SVG in chat, the bridge auto-downloads it to agent-creations/ and notifies you of the local relative path. Use that path in your code once reported.",
        "\nAVAILABLE CORE TOOLS:",
        "- View Directory Structure (ASCII tree):\n  <tool='view_dir'>src</tool>",
        "- Read File (Initial chunk with pagination metadata if large):\n  <tool='read' path='path/to/file.rs'></tool>",
        "- Read Specific Line Range:\n  <tool='read_lines' path='file.rs' start='10' end='40'></tool>",
        "- Search Codebase (Regex or exact substring):\n  <tool='search_code' query='fn calculate'>src</tool>",
        "- Write New / Overwrite Entire File:\n  <tool='write' path='path/to/file.rs'>\n[RAW CONTENT]\n</tool>",
        "- Surgical Replace Block (Bounded by lines):\n  <tool='replace' path='path/to/file.rs' start='10' end='25'>\n<search>\n[EXACT OLD TEXT]\n</search>\n<replace>\n[NEW REPLACEMENT TEXT]\n</replace>\n</tool>",
        "- Surgical Patch File:\n  <tool='patch' path='path/to/file.rs'>\n<search>\n[EXACT OLD TEXT]\n</search>\n<replace>\n[NEW REPLACEMENT TEXT]\n</replace>\n</tool>",
        "- Run Terminal Command (Auto-backgrounds after 5s if long-running):\n  <tool='terminal_run'>cargo build</tool>",
        "- Start Background Process (Returns PID):\n  <tool='terminal_bg'>npm run dev</tool>",
        "- Read Background Process Logs:\n  <tool='terminal_logs' pid='PID_HERE'></tool>",
        "- Terminate Background Process:\n  <tool='terminal_kill' pid='PID_HERE'></tool>",
        "- Monitor Background Process (Sleep and fetch logs):\n  <tool='cron_monitor' pid='PID_HERE' delay='15'></tool>",
    ].join("\n");

    let full_prompt = if mcp_prompt.is_empty() {
        format!("{}\n\nAcknowledge instructions strictly by replying: WebAgentMCP Ready.", base_prompt)
    } else {
        format!("{}\n\n{}\n\nAcknowledge instructions strictly by replying: WebAgentMCP Ready.", base_prompt, mcp_prompt)
    };

    Json(json!({ "prompt": full_prompt }))
}