use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::i18n::I18n;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub workspace_dir: String,
    pub port: u16,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub custom_instructions: String,
}

fn default_language() -> String {
    I18n::detect_system_language()
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_dir = std::env::current_dir()
            .map(|p| {
                let s = p.to_string_lossy().to_string();
                s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
            })
            .unwrap_or_else(|_| ".".to_string());

        Self {
            workspace_dir: default_dir,
            port: 5505,
            language: default_language(),
            mcp_servers: HashMap::new(),
            custom_instructions: String::new(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("WebAgentMCP").join("config.json"))
            .unwrap_or_else(|| PathBuf::from("config.json"))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                    cfg.workspace_dir = cfg.workspace_dir.strip_prefix(r"\\?\").unwrap_or(&cfg.workspace_dir).to_string();
                    return cfg;
                }
            }
        }
        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn set_workspace(&mut self, path: &str) -> Result<String, String> {
        let clean_input = path.strip_prefix(r"\\?\").unwrap_or(path);
        let p = Path::new(clean_input);
        if !p.exists() {
            return Err("Path does not exist".to_string());
        }
        if !p.is_dir() {
            return Err("Path is not a directory".to_string());
        }
        let canonical = p.canonicalize().map_err(|e| e.to_string())?;
        let abs_str = canonical.to_string_lossy().to_string();
        let clean_abs = abs_str.strip_prefix(r"\\?\").unwrap_or(&abs_str).to_string();
        self.workspace_dir = clean_abs.clone();
        let _ = self.save();
        Ok(clean_abs)
    }

    pub fn add_mcp_server(&mut self, name: String, cfg: McpServerConfig) -> Result<(), std::io::Error> {
        self.mcp_servers.insert(name, cfg);
        self.save()
    }

    pub fn remove_mcp_server(&mut self, name: &str) -> Result<bool, std::io::Error> {
        let removed = self.mcp_servers.remove(name).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn toggle_mcp_server(&mut self, name: &str, enabled: bool) -> Result<bool, std::io::Error> {
        if let Some(srv) = self.mcp_servers.get_mut(name) {
            srv.enabled = enabled;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}