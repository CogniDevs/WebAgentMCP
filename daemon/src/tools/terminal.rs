use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn paginate_output(workspace: &str, output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        return output.to_string();
    }

    let save_dir = Path::new(workspace).join(".webagentmcp");
    let _ = fs::create_dir_all(&save_dir);
    let dump_file = save_dir.join("last_output.txt");
    let _ = fs::write(&dump_file, output);

    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    if total_lines <= 1500 {
        return output.to_string();
    }

    let head = lines[..800.min(total_lines)].join("\n");
    let tail_start = total_lines.saturating_sub(400);
    let tail = lines[tail_start..].join("\n");

    format!(
        "{}\n\n[... {} lines omitted ...]\n\n{}\n\n[Showing lines 1-800 and {}-{} of {} total. Full log preserved in .webagentmcp/last_output.txt]",
        head,
        tail_start.saturating_sub(800),
        tail,
        tail_start + 1,
        total_lines,
        total_lines
    )
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub status: String,
    pub logs: VecDeque<String>,
}

#[derive(Clone)]
pub struct TerminalManager {
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
}

async fn stream_to_logs<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    pref: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    pid: String,
) {
    let mut buffer = [0u8; 4096];
    let mut leftover = String::new();
    while let Ok(n) = reader.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buffer[..n]);
        leftover.push_str(&chunk);
        while let Some(pos) = leftover.find('\n') {
            let line = leftover[..pos].trim_end_matches('\r').to_string();
            leftover = leftover[pos + 1..].to_string();
            let mut map = pref.lock().await;
            if let Some(proc) = map.get_mut(&pid) {
                if proc.logs.len() >= 1500 {
                    proc.logs.pop_front();
                }
                proc.logs.push_back(line);
            }
        }
    }
    if !leftover.is_empty() {
        let mut map = pref.lock().await;
        if let Some(proc) = map.get_mut(&pid) {
            proc.logs.push_back(leftover);
        }
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn build_shell_command(workspace: &str, cmd_str: &str) -> Command {
        #[cfg(target_os = "windows")]
        {
            let trimmed = cmd_str.trim();
            let is_unix_cmd = trimmed.starts_with("ls")
                || trimmed.starts_with("pwd")
                || trimmed.starts_with("cat ")
                || trimmed.starts_with("grep ")
                || trimmed.starts_with("rm -")
                || trimmed.starts_with("touch ");

            let mut ps_cmd = cmd_str.to_string();
            if trimmed.starts_with("ls") {
                ps_cmd = ps_cmd
                    .replace(" -la ", " -Force ")
                    .replace(" -al ", " -Force ")
                    .replace(" -l ", " ")
                    .replace(" -a ", " -Force ");
                if ps_cmd.ends_with(" -la") || ps_cmd.ends_with(" -al") || ps_cmd.ends_with(" -a") {
                    ps_cmd = ps_cmd.trim_end_matches(" -la").trim_end_matches(" -al").trim_end_matches(" -a").to_string();
                    ps_cmd.push_str(" -Force");
                } else if ps_cmd.ends_with(" -l") {
                    ps_cmd = ps_cmd.trim_end_matches(" -l").to_string();
                }
            }

            let mut cmd = if is_unix_cmd {
                let mut c = Command::new("powershell");
                c.args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd]);
                c
            } else {
                let mut c = Command::new("cmd");
                let full_cmd = format!("chcp 65001 >nul && {}", cmd_str);
                c.args(["/C", &full_cmd]);
                c
            };
            cmd.current_dir(Path::new(workspace))
                .env("PYTHONIOENCODING", "utf-8")
                .env("PYTHONUTF8", "1")
                .env("FORCE_COLOR", "0")
                .creation_flags(CREATE_NO_WINDOW);
            cmd
        }

        #[cfg(not(target_os = "windows"))]
        {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", cmd_str])
                .current_dir(Path::new(workspace))
                .env("LANG", "C.UTF-8")
                .env("LC_ALL", "C.UTF-8")
                .env("FORCE_COLOR", "0");
            cmd
        }
    }

    pub async fn run_sync(
        &self,
        workspace: &str,
        command_str: &str,
        quick_wait_secs: u64,
    ) -> Result<String, String> {
        let mut cmd = Self::build_shell_command(workspace, command_str);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        let pid_str = Uuid::new_v4().to_string()[..8].to_string();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let info = ProcessInfo {
            status: "running".to_string(),
            logs: VecDeque::with_capacity(1500),
        };

        {
            let mut procs = self.processes.lock().await;
            procs.insert(pid_str.clone(), info);
        }

        let processes_ref = self.processes.clone();
        let pid_clone = pid_str.clone();

        if let Some(out) = stdout {
            let pref = processes_ref.clone();
            let pid = pid_clone.clone();
            tokio::spawn(async move {
                stream_to_logs(out, pref, pid).await;
            });
        }

        if let Some(err) = stderr {
            let pref = processes_ref.clone();
            let pid = pid_clone.clone();
            tokio::spawn(async move {
                stream_to_logs(err, pref, pid).await;
            });
        }

        let pref_status = self.processes.clone();
        let pid_status = pid_str.clone();

        let wait_task = tokio::spawn(async move {
            let status_res = child.wait().await;
            let mut map = pref_status.lock().await;
            if let Some(proc) = map.get_mut(&pid_status) {
                proc.status = match status_res {
                    Ok(exit) => {
                        let code = exit.code().unwrap_or(0);
                        if code == 0 {
                            "exited (0)".to_string()
                        } else {
                            format!("exited ({}) [Error]", code)
                        }
                    }
                    Err(_) => "failed".to_string(),
                };
            }
        });

        let wait_duration = Duration::from_secs(quick_wait_secs.min(6));
        match tokio::time::timeout(wait_duration, wait_task).await {
            Ok(_) => {
                tokio::time::sleep(Duration::from_millis(150)).await;
                let logs = self.get_logs(&pid_str).await.unwrap_or_default();
                let mut map = self.processes.lock().await;
                map.remove(&pid_str);

                Ok(paginate_output(workspace, &logs, 24000))
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let current_logs = {
                    let map = self.processes.lock().await;
                    map.get(&pid_str)
                        .map(|p| p.logs.iter().cloned().collect::<Vec<_>>().join("\n"))
                        .unwrap_or_default()
                };

                let partial = paginate_output(workspace, &current_logs, 4000);
                Ok(format!(
                    "STATUS: ONGOING (Process auto-backgrounded after {}s)\nPID: {}\n\nInitial output:\n{}\n\nInstruction: This process is still running. Use <tool='terminal_logs' pid='{}'></tool> or <tool='cron_monitor' pid='{}' delay='15'></tool> to inspect progress, or <tool='terminal_kill' pid='{}'></tool> to stop.",
                    quick_wait_secs.min(6), pid_str, partial, pid_str, pid_str, pid_str
                ))
            }
        }
    }

    pub async fn run_bg(&self, workspace: &str, command_str: &str) -> Result<String, String> {
        let mut cmd = Self::build_shell_command(workspace, command_str);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn background process: {}", e))?;

        let pid_str = Uuid::new_v4().to_string()[..8].to_string();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let info = ProcessInfo {
            status: "running".to_string(),
            logs: VecDeque::with_capacity(1500),
        };

        {
            let mut procs = self.processes.lock().await;
            procs.insert(pid_str.clone(), info);
        }

        let processes_ref = self.processes.clone();
        let pid_clone = pid_str.clone();

        if let Some(out) = stdout {
            let pref = processes_ref.clone();
            let pid = pid_clone.clone();
            tokio::spawn(async move {
                stream_to_logs(out, pref, pid).await;
            });
        }

        if let Some(err) = stderr {
            let pref = processes_ref.clone();
            let pid = pid_clone.clone();
            tokio::spawn(async move {
                stream_to_logs(err, pref, pid).await;
            });
        }

        tokio::spawn(async move {
            let status_res = child.wait().await;
            let mut map = processes_ref.lock().await;
            if let Some(proc) = map.get_mut(&pid_clone) {
                proc.status = match status_res {
                    Ok(exit) => {
                        let code = exit.code().unwrap_or(0);
                        if code == 0 {
                            "exited (0)".to_string()
                        } else {
                            format!("exited ({}) [Error]", code)
                        }
                    }
                    Err(_) => "failed".to_string(),
                };
            }
        });

        Ok(format!(
            "Process started in background. PID: {}. Use <tool='terminal_logs' pid='{}'></tool> or <tool='cron_monitor' pid='{}' delay='15'></tool> to inspect output.",
            pid_str, pid_str, pid_str
        ))
    }

    pub async fn get_logs(&self, pid: &str) -> Result<String, String> {
        let map = self.processes.lock().await;
        match map.get(pid) {
            Some(proc) => {
                let lines: Vec<String> = proc.logs.iter().cloned().collect();
                let log_text = if lines.is_empty() {
                    "No output produced.".to_string()
                } else {
                    lines.join("\n")
                };
                Ok(format!("STATUS: {}\nOUTPUT:\n{}", proc.status, log_text))
            }
            None => Err(format!("Process with PID {} not found.", pid)),
        }
    }

    pub async fn kill(&self, pid: &str) -> Result<String, String> {
        let mut map = self.processes.lock().await;
        if let Some(proc) = map.get_mut(pid) {
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", pid])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .await;
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = Command::new("pkill")
                    .args(["-P", pid])
                    .output()
                    .await;
            }

            proc.status = "killed".to_string();
            Ok(format!("Process {} terminated.", pid))
        } else {
            Err(format!("Process with PID {} not found.", pid))
        }
    }
}