use colored::Colorize;
use rust_embed::RustEmbed;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::i18n::{init_i18n, t, t_fmt, I18n};

#[derive(RustEmbed)]
#[folder = "../extension/"]
pub struct EmbeddedExtension;

pub enum SetupOutcome {
    ContinueToDaemon,
    ExitNormally,
}

pub fn get_app_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".webagentmcp")
}

pub fn get_bin_dir() -> PathBuf {
    get_app_dir().join("bin")
}

pub fn get_extension_dir() -> PathBuf {
    get_app_dir().join("extension")
}

pub fn get_pid_file() -> PathBuf {
    get_app_dir().join("daemon.pid")
}

pub fn get_token_file() -> PathBuf {
    get_app_dir().join("auth.token")
}

pub fn get_or_create_auth_token() -> String {
    let token_path = get_token_file();
    let app_dir = get_app_dir();
    let _ = fs::create_dir_all(&app_dir);

    if token_path.exists() {
        if let Ok(content) = fs::read_to_string(&token_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let new_token = Uuid::new_v4().simple().to_string();
    let _ = fs::write(&token_path, &new_token);
    new_token
}

pub fn copy_clipboard(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(mut child) = Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            return child.wait().map(|s| s.success()).unwrap_or(false);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            return child.wait().map(|s| s.success()).unwrap_or(false);
        }
    }

    #[cfg(target_os = "linux")]
    {
        for cmd_name in ["wl-copy", "xclip", "xsel"] {
            let mut cmd = Command::new(cmd_name);
            if cmd_name == "xclip" {
                cmd.args(["-selection", "clipboard"]);
            } else if cmd_name == "xsel" {
                cmd.args(["--clipboard", "--input"]);
            }
            cmd.stdin(std::process::Stdio::piped());
            if let Ok(mut child) = cmd.spawn() {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                if child.wait().map(|s| s.success()).unwrap_or(false) {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(target_os = "windows")]
pub fn add_dir_to_user_path(bin_dir: &Path) -> Result<(), io::Error> {
    let bin_str = bin_dir.to_string_lossy();
    let script = format!(
        r#"$p = [Environment]::GetEnvironmentVariable('PATH', 'User'); if ($p -notlike '*{0}*') {{ [Environment]::SetEnvironmentVariable('PATH', "$p;{0}", 'User') }}"#,
        bin_str
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, t("failed_update_path")))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn add_dir_to_user_path(bin_dir: &Path) -> Result<(), io::Error> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let bin_str = bin_dir.to_string_lossy();
    let export_line = format!("\nexport PATH=\"$PATH:{}\"\n", bin_str);

    for rc in [".bashrc", ".zshrc", ".profile"] {
        let rc_path = home.join(rc);
        if rc_path.exists() {
            let content = fs::read_to_string(&rc_path).unwrap_or_default();
            if !content.contains(&*bin_str) {
                let mut f = fs::OpenOptions::new().append(true).open(&rc_path)?;
                f.write_all(export_line.as_bytes())?;
            }
        }
    }
    Ok(())
}

fn cleanup_old_binaries() {
    let bin_dir = get_bin_dir();
    if let Ok(entries) = fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("old") {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub fn self_install_binary() -> Result<PathBuf, io::Error> {
    println!("{}", t("step_bin_install").cyan());
    cleanup_old_binaries();

    let current_exe = std::env::current_exe()?;
    let bin_dir = get_bin_dir();
    fs::create_dir_all(&bin_dir)?;

    let exe_name = if cfg!(target_os = "windows") { "webagentmcp.exe" } else { "webagentmcp" };
    let alias_name = if cfg!(target_os = "windows") { "wamcp.exe" } else { "wamcp" };

    let target_path = bin_dir.join(exe_name);
    let alias_path = bin_dir.join(alias_name);

    if current_exe != target_path {
        if target_path.exists() {
            let _ = fs::rename(&target_path, target_path.with_extension("exe.old"));
        }
        if alias_path.exists() {
            let _ = fs::rename(&alias_path, alias_path.with_extension("exe.old"));
        }

        fs::copy(&current_exe, &target_path)?;
        let _ = fs::copy(&current_exe, &alias_path);
        println!("  ✓ {}", t_fmt("bin_installed_to", &[("path", &target_path.to_string_lossy())]).green());
    } else {
        println!("  ✓ {}", t("bin_already_installed"));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        let _ = fs::set_permissions(&target_path, perms.clone());
        let _ = fs::set_permissions(&alias_path, perms);
    }

    add_dir_to_user_path(&bin_dir)?;
    Ok(target_path)
}

pub fn extract_embedded_extension(dest: &Path) -> Result<(), io::Error> {
    println!("{}", t("step_ext_extract").cyan());
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    fs::create_dir_all(dest)?;

    let mut count = 0usize;
    for file in EmbeddedExtension::iter() {
        let file_path = dest.join(file.as_ref());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(content) = EmbeddedExtension::get(file.as_ref()) {
            fs::write(&file_path, content.data.as_ref())?;
            count += 1;
        }
    }

    let count_str = count.to_string();
    println!("  ✓ {}", t_fmt("ext_extracted_to", &[("path", &dest.to_string_lossy()), ("count", &count_str)]).green());
    Ok(())
}

fn open_browser(choice: &str) {
    #[cfg(target_os = "windows")]
    {
        let cmd = match choice {
            "1" => Some("chrome"),
            "2" => Some("msedge"),
            "3" => Some("brave"),
            "4" => Some("chromium"),
            "5" => Some("firefox"),
            _ => None,
        };

        if let Some(browser) = cmd {
            let _ = Command::new("cmd").args(["/C", "start", browser]).spawn();
        }
    }

    #[cfg(target_os = "macos")]
    {
        let app = match choice {
            "1" => Some("Google Chrome"),
            "2" => Some("Microsoft Edge"),
            "3" => Some("Brave Browser"),
            "4" => Some("Chromium"),
            "5" => Some("Firefox"),
            _ => None,
        };

        if let Some(browser) = app {
            let _ = Command::new("open").args(["-a", browser]).spawn();
        }
    }

    #[cfg(target_os = "linux")]
    {
        let bins = match choice {
            "1" => vec!["google-chrome", "google-chrome-stable"],
            "2" => vec!["microsoft-edge", "microsoft-edge-stable"],
            "3" => vec!["brave-browser", "brave"],
            "4" => vec!["chromium", "chromium-browser"],
            "5" => vec!["firefox"],
            _ => vec![],
        };

        for b in bins {
            if let Ok(mut child) = Command::new(b).spawn() {
                let _ = child.id();
                break;
            }
        }
    }
}

pub fn prompt_language_selection() -> String {
    let detected = I18n::detect_system_language();
    let langs = I18n::available_languages();

    println!("{}", "==================================================".bright_cyan());
    println!("  WebAgentMCP Setup & Maintenance Wizard");
    println!("{}", "==================================================".bright_cyan());
    println!("Detected system language: [{}]", detected.yellow());
    println!("Available languages:");
    for (idx, (code, label)) in langs.iter().enumerate() {
        println!("  {}. {} [{}]", idx + 1, label, code);
    }
    println!();
    print!("Select language [1-{}] (default: {}): ", langs.len(), detected);
    let _ = io::stdout().flush();

    let mut line = String::new();
    let stdin = io::stdin();
    if stdin.lock().read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return detected;
        }
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= langs.len() {
                return langs[num - 1].0.to_string();
            }
        }
        for (code, _) in &langs {
            if trimmed.eq_ignore_ascii_case(code) {
                return code.to_string();
            }
        }
    }
    detected
}

pub fn run_setup_wizard() -> Result<SetupOutcome, io::Error> {
    let mut config = AppConfig::load();
    let selected_lang = prompt_language_selection();
    init_i18n(&selected_lang);
    config.language = selected_lang;
    let _ = config.save();

    let ver = env!("CARGO_PKG_VERSION");
    println!();
    println!("{}", "==================================================".bright_cyan());
    println!("  {}", t_fmt("title_banner", &[("version", ver)]).bold().green());
    println!("{}", "==================================================".bright_cyan());

    println!("{}", t("menu_title").bold());
    println!("  {}", t("menu_opt_1"));
    println!("  {}", t("menu_opt_2"));
    println!("  {}", t("menu_opt_3"));
    println!("  {}", t("menu_opt_4"));
    println!("  {}", t("menu_opt_5"));
    println!();
    print!("{}", t("enter_choice"));
    let _ = io::stdout().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();
    let effective_choice = if choice.is_empty() { "1" } else { choice };

    match effective_choice {
        "1" => {
            let ext_dir = get_extension_dir();
            self_install_binary()?;
            extract_embedded_extension(&ext_dir)?;
            println!("{}", t("step_config_init").cyan());
            let _ = config.save();
            println!("  ✓ {}", t_fmt("config_written_to", &[("path", &AppConfig::config_path().to_string_lossy())]).green());
            run_browser_config_steps(&ext_dir)?;
        }
        "2" => {
            let ext_dir = get_extension_dir();
            extract_embedded_extension(&ext_dir)?;
            run_browser_config_steps(&ext_dir)?;
        }
        "3" => {
            self_install_binary()?;
        }
        "4" => {
            return Ok(SetupOutcome::ContinueToDaemon);
        }
        "5" => {
            std::process::exit(0);
        }
        _ => {
            println!("{}", t("unknown_option").red());
            std::process::exit(0);
        }
    }

    println!();
    println!("{}", "--------------------------------------------------".bright_black());
    println!("{}", t("setup_finished").bold().green());
    println!();
    print!("{}", t("press_enter_exit"));
    let _ = io::stdout().flush();
    let mut _pause = String::new();
    let _ = io::stdin().read_line(&mut _pause);

    Ok(SetupOutcome::ExitNormally)
}

fn run_browser_config_steps(ext_dir: &Path) -> Result<(), io::Error> {
    println!();
    println!("{}", t("browser_selection_title").bold());
    println!("  {}", t("browser_opt_1"));
    println!("  {}", t("browser_opt_2"));
    println!("  {}", t("browser_opt_3"));
    println!("  {}", t("browser_opt_4"));
    println!("  {}", t("browser_opt_5"));
    println!("  {}", t("browser_opt_6"));
    println!();
    print!("{}", t("browser_choice_prompt"));
    io::stdout().flush()?;

    let mut browser_input = String::new();
    io::stdin().read_line(&mut browser_input)?;
    let b_choice = browser_input.trim();
    let effective_b = if b_choice.is_empty() { "1" } else { b_choice };

    let copied = copy_clipboard(&ext_dir.to_string_lossy());
    let clipboard_status = if copied {
        t("clipboard_copied").green()
    } else {
        "".normal()
    };

    if effective_b != "6" {
        open_browser(effective_b);
    }

    println!();
    println!("{}", t("browser_instructions_title").bold());
    if effective_b == "5" {
        println!("{}", t_fmt("instructions_firefox", &[("path", &ext_dir.to_string_lossy())]));
    } else {
        println!("{}", t_fmt("instructions_chromium", &[("path", &ext_dir.to_string_lossy())]));
        if copied {
            println!("  {}", clipboard_status);
        }
    }

    Ok(())
}

pub fn ensure_installed_or_prompt() -> Result<SetupOutcome, io::Error> {
    let manifest = get_extension_dir().join("manifest.json");
    let bin_exe = get_bin_dir().join(if cfg!(target_os = "windows") { "webagentmcp.exe" } else { "webagentmcp" });

    if !manifest.exists() || !bin_exe.exists() {
        return run_setup_wizard();
    }
    Ok(SetupOutcome::ContinueToDaemon)
}