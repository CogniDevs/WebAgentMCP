use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::config::{AppConfig, McpServerConfig};
use crate::i18n::{init_i18n, t, t_fmt};
use crate::setup::{get_app_dir, get_or_create_auth_token, get_pid_file};

#[derive(Parser, Debug)]
#[command(name = "webagentmcp")]
#[command(author = "CogniDevs")]
#[command(version = "0.3.0")]
#[command(about = "Universal Web-to-Local Bridge & MCP Host for AI Chats")]
pub struct CliArgs {
    #[arg(short, long)]
    pub dir: Option<String>,

    #[arg(short, long)]
    pub port: Option<u16>,

    #[arg(long)]
    pub lang: Option<String>,

    #[arg(long)]
    pub tray: bool,

    #[arg(long, hide = true)]
    pub tray_internal: bool,

    #[command(subcommand)]
    pub command: Option<CliCommands>,
}

#[derive(Subcommand, Debug)]
pub enum CliCommands {
    Start {
        #[arg(short, long)]
        dir: Option<String>,

        #[arg(short, long)]
        port: Option<u16>,

        #[arg(long)]
        tray: bool,

        #[arg(long, hide = true)]
        tray_internal: bool,
    },

    Status,

    Stop,

    Workspace(WorkspaceArgs),

    Mcp(McpArgs),

    Setup,

    Uninstall,

    Completions {
        #[arg(value_enum)]
        shell: ShellType,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ShellType {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}

impl From<ShellType> for Shell {
    fn from(s: ShellType) -> Self {
        match s {
            ShellType::Bash => Shell::Bash,
            ShellType::Elvish => Shell::Elvish,
            ShellType::Fish => Shell::Fish,
            ShellType::Powershell => Shell::PowerShell,
            ShellType::Zsh => Shell::Zsh,
        }
    }
}

#[derive(Args, Debug)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub action: Option<WorkspaceAction>,
}

#[derive(Subcommand, Debug)]
pub enum WorkspaceAction {
    Get,
    Set {
        path: String,
    },
}

#[derive(Args, Debug)]
pub struct McpArgs {
    #[command(subcommand)]
    pub action: McpAction,
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    List,
    Add {
        name: String,
        command: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Toggle {
        name: String,
        enable: String,
    },
    Remove {
        name: String,
    },
}

fn post_local(port: u16, path: &str, token: &str) -> bool {
    use std::io::Write;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\n\r\n",
        path, token
    );
    if let Ok(mut stream) = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)) {
        return stream.write_all(request.as_bytes()).is_ok();
    }
    false
}

#[cfg(target_os = "windows")]
fn spawn_detached_tray(dir: Option<&str>, port: Option<u16>, lang: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let current_exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(current_exe);
    cmd.arg("start").arg("--tray-internal");

    if let Some(d) = dir {
        cmd.arg("-d").arg(d);
    }
    if let Some(p) = port {
        cmd.arg("-p").arg(p.to_string());
    }
    if let Some(l) = lang {
        cmd.arg("--lang").arg(l);
    }

    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    cmd.spawn()?;

    println!("{}", t("daemon_tray_started").green());
    println!("{}", t("daemon_tray_hint").dimmed());
    std::process::exit(0);
}

#[cfg(not(target_os = "windows"))]
fn spawn_detached_tray(dir: Option<&str>, port: Option<u16>, lang: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(current_exe);
    cmd.arg("start").arg("--tray-internal");

    if let Some(d) = dir {
        cmd.arg("-d").arg(d);
    }
    if let Some(p) = port {
        cmd.arg("-p").arg(p.to_string());
    }
    if let Some(l) = lang {
        cmd.arg("--lang").arg(l);
    }

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.spawn()?;

    println!("{}", t("daemon_tray_started").green());
    println!("{}", t("daemon_tray_hint").dimmed());
    std::process::exit(0);
}

pub fn handle_cli_pre_exec(args: &CliArgs) -> Result<bool, Box<dyn std::error::Error>> {
    let mut config = AppConfig::load();
    if let Some(ref l) = args.lang {
        config.language = l.clone();
        let _ = config.save();
    }
    init_i18n(&config.language);

    let want_tray = args.tray || matches!(args.command, Some(CliCommands::Start { tray: true, .. }));
    let is_tray_internal = args.tray_internal || matches!(args.command, Some(CliCommands::Start { tray_internal: true, .. }));

    if want_tray && !is_tray_internal {
        let explicit_dir = match &args.command {
            Some(CliCommands::Start { dir, .. }) => dir.as_deref(),
            _ => args.dir.as_deref(),
        };
        let explicit_port = match &args.command {
            Some(CliCommands::Start { port, .. }) => *port,
            _ => args.port,
        };
        spawn_detached_tray(explicit_dir, explicit_port, args.lang.as_deref())?;
        return Ok(true);
    }

    if let Some(cmd) = &args.command {
        match cmd {
            CliCommands::Start { .. } => {
                return Ok(false);
            }
            CliCommands::Status => {
                let cfg = AppConfig::load();
                println!("{}", t("status_title").bold().cyan());
                println!("{}", t_fmt("status_workspace", &[("path", &cfg.workspace_dir)]).green());
                let port_str = cfg.port.to_string();
                println!("{}", t_fmt("status_port", &[("port", &port_str)]).yellow());
                let config_str = AppConfig::config_path().to_string_lossy().to_string();
                println!("{}", t_fmt("status_config", &[("path", &config_str)]).dimmed());

                let active_servers: Vec<_> = cfg.mcp_servers.iter().filter(|(_, s)| s.enabled).collect();
                let total_s = cfg.mcp_servers.len().to_string();
                let active_s = active_servers.len().to_string();
                println!(
                    "{}",
                    t_fmt("status_mcp_servers", &[("total", &total_s), ("active", &active_s)])
                );
                return Ok(true);
            }
            CliCommands::Stop => {
                let pid_file = get_pid_file();
                let mut stopped = false;

                if pid_file.exists() {
                    if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                        let pid = pid_str.trim();
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("taskkill")
                                .args(["/F", "/PID", pid])
                                .output();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = std::process::Command::new("kill")
                                .args(["-9", pid])
                                .output();
                        }
                        let _ = fs::remove_file(&pid_file);
                        println!("{}", t_fmt("daemon_stopped_success", &[("pid", pid)]).green());
                        stopped = true;
                    }
                }

                if !stopped {
                    let token = get_or_create_auth_token();
                    if post_local(config.port, "/api/shutdown", &token) {
                        println!("{}", t("daemon_stop_sent").green());
                    } else {
                        println!("{}", t("daemon_not_running").yellow());
                    }
                }
                return Ok(true);
            }
            CliCommands::Workspace(ws_args) => {
                let mut cfg = AppConfig::load();
                match &ws_args.action {
                    None | Some(WorkspaceAction::Get) => {
                        println!("{}", t_fmt("workspace_current", &[("path", &cfg.workspace_dir)]).green());
                    }
                    Some(WorkspaceAction::Set { path }) => {
                        match cfg.set_workspace(path) {
                            Ok(new_dir) => {
                                println!("{}", t_fmt("workspace_changed", &[("path", &new_dir)]).green());
                            }
                            Err(e) => {
                                eprintln!("{}", t_fmt("workspace_error", &[("error", &e)]).red());
                            }
                        }
                    }
                }
                return Ok(true);
            }
            CliCommands::Mcp(mcp_args) => {
                let mut config = AppConfig::load();
                match &mcp_args.action {
                    McpAction::List => {
                        println!("{}", t("mcp_list_title").bold().cyan());
                        if config.mcp_servers.is_empty() {
                            println!("{}", t("mcp_no_servers"));
                        } else {
                            for (name, srv) in &config.mcp_servers {
                                let status = if srv.enabled { "ACTIVE".green() } else { "DISABLED".red() };
                                println!("- {} [{}]: {} {}", name.bold(), status, srv.command, srv.args.join(" "));
                            }
                        }
                    }
                    McpAction::Add { name, command, args } => {
                        let new_srv = McpServerConfig {
                            command: command.clone(),
                            args: args.clone(),
                            env: HashMap::new(),
                            enabled: true,
                        };
                        config.add_mcp_server(name.clone(), new_srv)?;
                        println!("{}", t_fmt("mcp_added", &[("name", name)]).green());
                    }
                    McpAction::Toggle { name, enable } => {
                        let is_on = match enable.to_lowercase().as_str() {
                            "true" | "1" | "on" | "yes" | "enable" => true,
                            _ => false,
                        };
                        if config.toggle_mcp_server(name, is_on)? {
                            let text = if is_on { "enabled" } else { "disabled" };
                            println!("{}", t_fmt("mcp_toggled", &[("name", name), ("status", text)]).green());
                        } else {
                            println!("{}", t_fmt("mcp_not_found", &[("name", name)]).red());
                        }
                    }
                    McpAction::Remove { name } => {
                        if config.remove_mcp_server(name)? {
                            println!("{}", t_fmt("mcp_removed", &[("name", name)]).green());
                        } else {
                            println!("{}", t_fmt("mcp_not_found", &[("name", name)]).red());
                        }
                    }
                }
                return Ok(true);
            }
            CliCommands::Setup => {
                crate::setup::run_setup_wizard()?;
                return Ok(true);
            }
            CliCommands::Uninstall => {
                let app_dir = get_app_dir();
                if !app_dir.exists() {
                    println!("{}", t("uninstall_not_found").yellow());
                    return Ok(true);
                }

                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/IM", "webagentmcp.exe"])
                        .output();
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/IM", "webagentmcp-daemon.exe"])
                        .output();

                    let pid_file = get_pid_file();
                    let _ = fs::remove_file(pid_file);

                    let app_dir_str = app_dir.to_string_lossy().to_string();
                    let script = format!(
                        "Start-Sleep -Milliseconds 600; Remove-Item -Recurse -Force -LiteralPath '{}'",
                        app_dir_str
                    );

                    let _ = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                        .spawn();

                    println!("{}", t("uninstall_success").green());
                    std::process::exit(0);
                }

                #[cfg(not(target_os = "windows"))]
                {
                    fs::remove_dir_all(&app_dir)?;
                    println!("{}", t("uninstall_success").green());
                    return Ok(true);
                }
            }
            CliCommands::Completions { shell } => {
                let mut cmd = CliArgs::command();
                let bin_name = cmd.get_name().to_string();
                let clap_shell: Shell = (*shell).into();
                generate(clap_shell, &mut cmd, bin_name, &mut io::stdout());
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn strip_unc_prefix(path_str: &str) -> String {
    path_str.strip_prefix(r"\\?\").unwrap_or(path_str).to_string()
}

pub fn resolve_effective_workspace(cli_dir: Option<&str>, config_dir: &str) -> String {
    if let Some(explicit) = cli_dir {
        let p = Path::new(explicit);
        if let Ok(canon) = p.canonicalize() {
            return strip_unc_prefix(&canon.to_string_lossy());
        }
        return strip_unc_prefix(explicit);
    }

    if let Ok(curr) = std::env::current_dir() {
        if let Ok(canon) = curr.canonicalize() {
            return strip_unc_prefix(&canon.to_string_lossy());
        }
        return strip_unc_prefix(&curr.to_string_lossy());
    }

    strip_unc_prefix(config_dir)
}