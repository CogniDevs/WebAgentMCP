pub mod fs;
pub mod terminal;

pub use terminal::TerminalManager;

use regex::Regex;

pub struct ToolDispatcher;

impl ToolDispatcher {
    pub async fn dispatch(
        workspace: &str,
        raw_name: &str,
        payload: &str,
        terminal: &TerminalManager,
    ) -> Result<String, String> {
        let combined = format!("<tool {} >{}</tool>", raw_name, payload);
        let tool_name = raw_name
            .split(|c: char| c.is_whitespace() || c == '\'' || c == '"')
            .next()
            .unwrap_or(raw_name)
            .trim();

        match tool_name {
            "view_dir" => {
                let sub = payload.trim();
                fs::view_dir(workspace, sub)
            }
            "read" | "read_file" => {
                let path_re = Regex::new(r#"path=['"]([^'"]+)['"]"#).unwrap();
                let path = path_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                fs::read_file(workspace, path)
            }
            "read_lines" => {
                let path_re = Regex::new(r#"path=['"]([^'"]+)['"]"#).unwrap();
                let start_re = Regex::new(r#"start=['"](\d+)['"]"#).unwrap();
                let end_re = Regex::new(r#"end=['"](\d+)['"]"#).unwrap();

                let path = path_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                let start = start_re
                    .captures(&combined)
                    .and_then(|c| c.get(1).unwrap().as_str().parse::<usize>().ok())
                    .unwrap_or(1);

                let end = end_re
                    .captures(&combined)
                    .and_then(|c| c.get(1).unwrap().as_str().parse::<usize>().ok())
                    .unwrap_or(100);

                fs::read_lines(workspace, path, start, end)
            }
            "write" => {
                let path_re = Regex::new(r#"path=['"]([^'"]+)['"]"#).unwrap();
                let path = path_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .ok_or_else(|| "Missing 'path' attribute in write tool".to_string())?;

                let content = payload.trim_start_matches('\n');
                fs::write_file(workspace, path, content)
            }
            "patch" => {
                let path_re = Regex::new(r#"path=['"]([^'"]+)['"]"#).unwrap();
                let search_re = Regex::new(r#"(?s)<search>(.*?)</search>"#).unwrap();
                let replace_re = Regex::new(r#"(?s)<replace>(.*?)</replace>"#).unwrap();

                let path = path_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .ok_or_else(|| "Missing 'path' attribute in patch tool".to_string())?;

                let search = search_re
                    .captures(payload)
                    .map(|c| c.get(1).unwrap().as_str().trim_matches('\n'))
                    .ok_or_else(|| "Missing <search> block in patch tool".to_string())?;

                let replace = replace_re
                    .captures(payload)
                    .map(|c| c.get(1).unwrap().as_str().trim_matches('\n'))
                    .ok_or_else(|| "Missing <replace> block in patch tool".to_string())?;

                fs::patch_file(workspace, path, search, replace)
            }
            "replace" => {
                let path_re = Regex::new(r#"path=['"]([^'"]+)['"]"#).unwrap();
                let start_re = Regex::new(r#"start=['"](\d+)['"]"#).unwrap();
                let end_re = Regex::new(r#"end=['"](\d+)['"]"#).unwrap();
                let search_re = Regex::new(r#"(?s)<search>(.*?)</search>"#).unwrap();
                let replace_re = Regex::new(r#"(?s)<replace>(.*?)</replace>"#).unwrap();

                let path = path_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .ok_or_else(|| "Missing 'path' attribute in replace tool".to_string())?;

                let start = start_re
                    .captures(&combined)
                    .and_then(|c| c.get(1).unwrap().as_str().parse::<usize>().ok())
                    .unwrap_or(0);

                let end = end_re
                    .captures(&combined)
                    .and_then(|c| c.get(1).unwrap().as_str().parse::<usize>().ok())
                    .unwrap_or(0);

                let search = search_re
                    .captures(payload)
                    .map(|c| c.get(1).unwrap().as_str().trim_matches('\n'))
                    .ok_or_else(|| "Missing <search> block in replace tool".to_string())?;

                let replace = replace_re
                    .captures(payload)
                    .map(|c| c.get(1).unwrap().as_str().trim_matches('\n'))
                    .ok_or_else(|| "Missing <replace> block in replace tool".to_string())?;

                fs::replace_bounded(workspace, path, start, end, search, replace)
            }
            "search_code" => {
                let query_re = Regex::new(r#"query=['"]([^'"]+)['"]"#).unwrap();
                let query = query_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                let sub_dir = if query == payload.trim() {
                    ""
                } else {
                    payload.trim()
                };

                fs::search_code(workspace, sub_dir, query)
            }
            "terminal_run" => {
                terminal.run_sync(workspace, payload.trim(), 5).await
            }
            "terminal_bg" => {
                terminal.run_bg(workspace, payload.trim()).await
            }
            "terminal_logs" => {
                let pid_re = Regex::new(r#"pid=['"]([^'"]+)['"]"#).unwrap();
                let pid = pid_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                terminal.get_logs(pid).await
            }
            "terminal_kill" => {
                let pid_re = Regex::new(r#"pid=['"]([^'"]+)['"]"#).unwrap();
                let pid = pid_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                terminal.kill(pid).await
            }
            "cron_monitor" => {
                let pid_re = Regex::new(r#"pid=['"]([^'"]+)['"]"#).unwrap();
                let delay_re = Regex::new(r#"delay=['"](\d+)['"]"#).unwrap();

                let pid = pid_re
                    .captures(&combined)
                    .map(|c| c.get(1).unwrap().as_str())
                    .unwrap_or(payload.trim());

                let delay = delay_re
                    .captures(&combined)
                    .and_then(|c| c.get(1).unwrap().as_str().parse::<u64>().ok())
                    .unwrap_or(15);

                let safe_delay = delay.min(120);
                tokio::time::sleep(std::time::Duration::from_secs(safe_delay)).await;
                let logs = terminal.get_logs(pid).await?;
                Ok(format!("[WOKE UP AFTER {}s]\n{}", safe_delay, logs))
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}