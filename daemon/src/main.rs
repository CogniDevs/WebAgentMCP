mod cli;
mod config;
mod i18n;
mod mcp;
mod server;
mod setup;
mod tools;

use clap::Parser;
use colored::Colorize;
use config::AppConfig;
use i18n::{init_i18n, t, t_fmt};
use mcp::McpManager;
use server::{create_router, AppState};
use setup::{get_or_create_auth_token, SetupOutcome};
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;
use tools::TerminalManager;
use cli::{CliArgs, handle_cli_pre_exec, resolve_effective_workspace};

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn PeekMessageA(lpMsg: *mut Msg, hWnd: usize, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> i32;
    fn TranslateMessage(lpMsg: *const Msg) -> i32;
    fn DispatchMessageA(lpMsg: *const Msg) -> isize;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn FreeConsole() -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct Msg {
    hwnd: usize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

fn hide_console() {
    #[cfg(target_os = "windows")]
    unsafe {
        let _ = FreeConsole();
    }
}

fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", url]).spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn post_local(port: u16, path: &str, token: &str) {
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\n\r\n",
        path, token
    );
    if let Ok(mut stream) = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)) {
        use std::io::Write;
        let _ = stream.write_all(request.as_bytes());
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    let mut config = AppConfig::load();
    if let Some(ref l) = args.lang {
        config.language = l.clone();
        let _ = config.save();
    }
    init_i18n(&config.language);

    if handle_cli_pre_exec(&args)? {
        return Ok(());
    }

    let is_tray_internal = args.tray_internal || matches!(args.command, Some(cli::CliCommands::Start { tray_internal: true, .. }));

    if is_tray_internal {
        hide_console();
    } else {
        match setup::ensure_installed_or_prompt()? {
            SetupOutcome::ExitNormally => return Ok(()),
            SetupOutcome::ContinueToDaemon => {}
        }
    }

    let explicit_dir = match &args.command {
        Some(cli::CliCommands::Start { dir, .. }) => dir.clone(),
        _ => args.dir.clone(),
    };

    let explicit_port = match &args.command {
        Some(cli::CliCommands::Start { port, .. }) => *port,
        _ => args.port,
    };

    let effective_ws = resolve_effective_workspace(explicit_dir.as_deref(), &config.workspace_dir);
    let _ = config.set_workspace(&effective_ws);

    if let Some(p) = explicit_port {
        config.port = p;
        let _ = config.save();
    }

    let port = config.port;
    let workspace = config.workspace_dir.clone();
    let auth_token = get_or_create_auth_token();

    let pid_file = setup::get_pid_file();
    let current_pid = std::process::id().to_string();
    let _ = fs::write(&pid_file, current_pid);

    let rt = tokio::runtime::Runtime::new()?;
    let daemon_token = auth_token.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    let config_clone = config.clone();
    let daemon_token_clone = daemon_token.clone();
    let workspace_clone = workspace.clone();

    rt.spawn(async move {
        let terminal = TerminalManager::new();
        let mcp = McpManager::init(&config_clone).await;

        let active_mcp = mcp.active_servers().await;
        let tools_count = mcp.list_tools().await.len();

        let ver = env!("CARGO_PKG_VERSION");
        if !is_tray_internal {
            println!("{}", "==================================================".bright_cyan());
            println!("  {}", format!("WebAgentMCP Daemon v{}", ver).bold().green());
            println!("  {}", "Universal Web-to-Local Bridge & MCP Host".dimmed());
            println!("{}", "==================================================".bright_cyan());
            println!("  {}: http://127.0.0.1:{}", "Endpoint".bold(), port);
            println!("  {}: {}", "Auth Key".bold(), daemon_token_clone.bright_yellow());
            println!("  {}: {}", "Workspace".bold(), workspace_clone);
            println!("  {}: {}", "Language".bold(), config_clone.language);
            println!(
                "  {}: {} active ({} tools loaded)",
                "MCP Servers".bold(),
                active_mcp.len(),
                tools_count
            );
            println!("{}", "--------------------------------------------------".bright_black());
            println!("  {}", t("daemon_running_wait").green());
            println!();
        }

        let state = AppState {
            config: Arc::new(Mutex::new(config_clone)),
            terminal,
            mcp,
            auth_token: daemon_token_clone,
        };

        let app = create_router(state);
        let addr = format!("127.0.0.1:{}", port);

        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                let _ = ready_tx.send(Ok(()));
                let _ = axum::serve(listener, app).await;
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
            }
        }
    });

    let bind_result = rt.block_on(ready_rx);
    match bind_result {
        Ok(Ok(())) => {}
        Ok(Err(err_msg)) => {
            eprintln!("{}", t_fmt("daemon_port_error", &[("port", &port.to_string()), ("error", &err_msg)]).red().bold());
            let _ = fs::remove_file(setup::get_pid_file());
            std::process::exit(1);
        }
        Err(_) => {
            let _ = fs::remove_file(setup::get_pid_file());
            std::process::exit(1);
        }
    }

    if is_tray_internal {
        run_tray_loop(port, &auth_token)?;
    } else {
        rt.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("\n{}", t("daemon_stopping").yellow());
        });
    }

    let _ = fs::remove_file(setup::get_pid_file());
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_tray_loop(port: u16, auth_token: &str) -> Result<(), Box<dyn std::error::Error>> {
    use muda::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let menu = Menu::new();
    let item_open = MenuItem::new("Open AI Chat", true, None);
    let item_dir = MenuItem::new("Choose Workspace Folder...", true, None);
    let item_status = MenuItem::new("Status", true, None);
    let item_quit = MenuItem::new("Quit WebAgentMCP", true, None);

    menu.append(&item_open)?;
    menu.append(&item_dir)?;
    menu.append(&item_status)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_quit)?;

    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 8..24 {
        for x in 8..24 {
            let idx = ((y * width + x) * 4) as usize;
            rgba[idx] = 35;
            rgba[idx + 1] = 134;
            rgba[idx + 2] = 54;
            rgba[idx + 3] = 255;
        }
    }
    let icon = Icon::from_rgba(rgba, width, height)?;

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("WebAgentMCP")
        .with_icon(icon)
        .build()?;

    let menu_channel = muda::MenuEvent::receiver();
    let mut win_msg = std::mem::MaybeUninit::<Msg>::uninit();

    loop {
        unsafe {
            while PeekMessageA(win_msg.as_mut_ptr(), 0, 0, 0, 1) != 0 {
                TranslateMessage(win_msg.as_ptr());
                DispatchMessageA(win_msg.as_ptr());
            }
        }

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == item_open.id() {
                open_url("https://aistudio.google.com/");
            } else if event.id == item_dir.id() {
                post_local(port, "/api/workspace/pick", auth_token);
            } else if event.id == item_status.id() {
                open_url(&format!("http://127.0.0.1:{}/api/status", port));
            } else if event.id == item_quit.id() {
                let _ = fs::remove_file(setup::get_pid_file());
                std::process::exit(0);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(not(target_os = "windows"))]
fn run_tray_loop(port: u16, auth_token: &str) -> Result<(), Box<dyn std::error::Error>> {
    use muda::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let menu = Menu::new();
    let item_open = MenuItem::new("Open AI Chat", true, None);
    let item_dir = MenuItem::new("Choose Workspace Folder...", true, None);
    let item_status = MenuItem::new("Status", true, None);
    let item_quit = MenuItem::new("Quit WebAgentMCP", true, None);

    menu.append(&item_open)?;
    menu.append(&item_dir)?;
    menu.append(&item_status)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_quit)?;

    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 8..24 {
        for x in 8..24 {
            let idx = ((y * width + x) * 4) as usize;
            rgba[idx] = 35;
            rgba[idx + 1] = 134;
            rgba[idx + 2] = 54;
            rgba[idx + 3] = 255;
        }
    }
    let icon = Icon::from_rgba(rgba, width, height)?;

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("WebAgentMCP")
        .with_icon(icon)
        .build()?;

    let menu_channel = muda::MenuEvent::receiver();

    loop {
        if let Ok(event) = menu_channel.try_recv() {
            if event.id == item_open.id() {
                open_url("https://aistudio.google.com/");
            } else if event.id == item_dir.id() {
                post_local(port, "/api/workspace/pick", auth_token);
            } else if event.id == item_status.id() {
                open_url(&format!("http://127.0.0.1:{}/api/status", port));
            } else if event.id == item_quit.id() {
                let _ = fs::remove_file(setup::get_pid_file());
                std::process::exit(0);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}