use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

pub struct McpClient {
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    request_id: u64,
}

impl McpClient {
    pub async fn start(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, String> {
        let mut cmd = if cfg!(target_os = "windows") && (command == "npx" || command == "npm") {
            let mut c = Command::new("cmd");
            let mut full_args = vec!["/C".to_string(), command.to_string()];
            full_args.extend_from_slice(args);
            c.args(full_args);
            c
        } else {
            let mut c = Command::new(command);
            c.args(args);
            c
        };

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.envs(env);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start MCP server {}: {}", command, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture stdin of MCP process".to_string())?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout of MCP process".to_string())?;

        let lines = BufReader::new(stdout).lines();

        let mut client = Self {
            _child: child,
            stdin,
            lines,
            request_id: 0,
        };

        client.initialize().await?;
        Ok(client)
    }

    async fn send_raw(&mut self, payload: Value) -> Result<(), String> {
        let serialized = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        let line = format!("{}\n", serialized);
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to MCP stdin: {}", e))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush MCP stdin: {}", e))?;
        Ok(())
    }

    async fn read_response(&mut self, expected_id: u64) -> Result<Value, String> {
        while let Ok(Some(line)) = self.lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                if let Some(id) = val.get("id").and_then(|i| i.as_u64()) {
                    if id == expected_id {
                        if let Some(err) = val.get("error") {
                            return Err(err.to_string());
                        }
                        return val.get("result").cloned().ok_or_else(|| {
                            "Missing 'result' field in JSON-RPC response".to_string()
                        });
                    }
                }
            }
        }
        Err("MCP process connection closed unexpectedly".to_string())
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request_id += 1;
        let current_id = self.request_id;
        let req = json!({
            "jsonrpc": "2.0",
            "id": current_id,
            "method": method,
            "params": params
        });

        self.send_raw(req).await?;
        self.read_response(current_id).await
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_raw(notification).await
    }

    async fn initialize(&mut self) -> Result<(), String> {
        let init_params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "clientInfo": {
                "name": "WebAgentMCP",
                "version": "0.1.0"
            }
        });

        self.request("initialize", init_params).await?;
        self.notify("notifications/initialized", json!({})).await?;
        Ok(())
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>, String> {
        let res = self.request("tools/list", json!({})).await?;
        let tools_raw = res
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        let mut result = Vec::new();
        for item in tools_raw {
            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                let desc = item
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
                let schema = item.get("inputSchema").cloned().unwrap_or(json!({}));
                result.push(McpTool {
                    name: name.to_string(),
                    description: desc,
                    input_schema: schema,
                });
            }
        }
        Ok(result)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String, String> {
        let params = json!({
            "name": name,
            "arguments": arguments
        });

        let res = self.request("tools/call", params).await?;
        if let Some(content_arr) = res.get("content").and_then(|c| c.as_array()) {
            let mut texts = Vec::new();
            for entry in content_arr {
                if let Some(txt) = entry.get("text").and_then(|t| t.as_str()) {
                    texts.push(txt.to_string());
                }
            }
            if !texts.is_empty() {
                return Ok(texts.join("\n"));
            }
        }

        Ok(res.to_string())
    }
}