pub mod client;

pub use client::{McpClient, McpTool};

use crate::config::{AppConfig, McpServerConfig};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct McpManager {
    clients: Arc<Mutex<HashMap<String, McpClient>>>,
    tool_to_server: Arc<Mutex<HashMap<String, String>>>,
    tools_list: Arc<Mutex<Vec<McpTool>>>,
}

impl McpManager {
    pub async fn init(config: &AppConfig) -> Self {
        let manager = Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            tool_to_server: Arc::new(Mutex::new(HashMap::new())),
            tools_list: Arc::new(Mutex::new(Vec::new())),
        };

        manager.reload_all(config).await;
        manager
    }

    pub async fn reload_all(&self, config: &AppConfig) {
        let mut clients_map = HashMap::new();
        let mut mapping = HashMap::new();
        let mut all_tools = Vec::new();

        for (server_name, srv_cfg) in &config.mcp_servers {
            if !srv_cfg.enabled {
                continue;
            }

            match McpClient::start(&srv_cfg.command, &srv_cfg.args, &srv_cfg.env).await {
                Ok(mut client) => {
                    if let Ok(tools) = client.list_tools().await {
                        for tool in &tools {
                            mapping.insert(tool.name.clone(), server_name.clone());
                            all_tools.push(tool.clone());
                        }
                    }
                    clients_map.insert(server_name.clone(), client);
                }
                Err(_) => continue,
            }
        }

        let mut clients_guard = self.clients.lock().await;
        *clients_guard = clients_map;

        let mut mapping_guard = self.tool_to_server.lock().await;
        *mapping_guard = mapping;

        let mut tools_guard = self.tools_list.lock().await;
        *tools_guard = all_tools;
    }

    pub async fn add_and_start(
        &self,
        name: String,
        cfg: McpServerConfig,
    ) -> Result<usize, String> {
        if !cfg.enabled {
            return Ok(0);
        }

        let mut client = McpClient::start(&cfg.command, &cfg.args, &cfg.env).await?;
        let tools = client.list_tools().await.unwrap_or_default();
        let tools_count = tools.len();

        let mut mapping_guard = self.tool_to_server.lock().await;
        let mut tools_guard = self.tools_list.lock().await;

        for tool in &tools {
            mapping_guard.insert(tool.name.clone(), name.clone());
            tools_guard.push(tool.clone());
        }

        let mut clients_guard = self.clients.lock().await;
        clients_guard.insert(name, client);

        Ok(tools_count)
    }

    pub async fn remove(&self, name: &str) {
        let mut clients_guard = self.clients.lock().await;
        clients_guard.remove(name);

        let mut mapping_guard = self.tool_to_server.lock().await;
        mapping_guard.retain(|_, srv| srv != name);

        let mut tools_guard = self.tools_list.lock().await;
        tools_guard.retain(|t| {
            if let Some(srv) = mapping_guard.get(&t.name) {
                srv != name
            } else {
                false
            }
        });
    }

    pub async fn list_tools(&self) -> Vec<McpTool> {
        let list = self.tools_list.lock().await;
        list.clone()
    }

    pub async fn active_servers(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect()
    }

    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<String, String> {
        let server_name = {
            let map = self.tool_to_server.lock().await;
            map.get(tool_name)
                .cloned()
                .ok_or_else(|| format!("MCP Tool '{}' not found in any connected server", tool_name))?
        };

        let mut clients = self.clients.lock().await;
        let client = clients
            .get_mut(&server_name)
            .ok_or_else(|| format!("MCP Server '{}' is not available", server_name))?;

        client.call_tool(tool_name, arguments).await
    }

    pub async fn generate_tools_prompt(&self) -> String {
        let tools = self.tools_list.lock().await;
        if tools.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();
        lines.push("=== EXTERNAL MCP TOOLS ===".to_string());
        lines.push("You also have access to these external MCP tools configured by the user:".to_string());

        for tool in tools.iter() {
            let desc = tool.description.as_deref().unwrap_or("No description provided.");
            let schema_str = serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".to_string());
            lines.push(format!("- Tool: {}\n  Description: {}\n  Schema: {}", tool.name, desc, schema_str));
        }

        lines.push("\nInvoke MCP tools using: <tool='mcp_call' name='TOOL_NAME'>{\"param\": \"value\"}</tool>".to_string());
        lines.join("\n")
    }
}