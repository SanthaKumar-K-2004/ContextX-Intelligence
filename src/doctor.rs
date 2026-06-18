use crate::cli::{DoctorArgs, SetupArgs};
use crate::setup;
use anyhow::Result;
use serde_json::json;
use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;

pub async fn run(args: DoctorArgs) -> Result<()> {
    if args.json {
        let payload = json!({
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "privacy_default": "memory-only",
            "network_default": "localhost-only",
            "telemetry": "disabled",
            "commands": {
                "claude": command_path_string("claude"),
                "codex": command_path_string("codex"),
                "aider": command_path_string("aider"),
                "cursor": command_path_string("cursor"),
                "code": command_path_string("code")
            },
            "env": {
                "ANTHROPIC_API_KEY": env::var_os("ANTHROPIC_API_KEY").is_some(),
                "OPENAI_API_KEY": env::var_os("OPENAI_API_KEY").is_some()
            },
            "port_8787_available": port_available(8787),
            "configs": {
                "claude_desktop": claude_desktop_config().map(|path| path.display().to_string()),
                "cursor_mcp": home_path(".cursor/mcp.json").map(|path| path.display().to_string()),
                "vscode": vscode_settings().map(|path| path.display().to_string())
            }
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    println!("ContextX Doctor");
    println!("=================");
    println!("Santhakumar K • Alpha X Solutions");
    println!("OS: {} {}", env::consts::OS, env::consts::ARCH);
    println!("Privacy default: memory-only");
    println!("Network default: localhost-only");
    println!("Telemetry: disabled");
    println!();

    check_command("claude");
    check_command("codex");
    check_command("aider");
    check_command("cursor");
    check_command("code");
    check_env("ANTHROPIC_API_KEY");
    check_env("OPENAI_API_KEY");
    check_port(8787);
    check_config("Claude Desktop", claude_desktop_config());
    check_config("Cursor MCP", home_path(".cursor/mcp.json"));
    check_config("VS Code user settings", vscode_settings());
    println!();
    println!("Recommended next steps:");
    if command_path("contextx").is_ok() {
        println!("  contextx setup --all --dry-run");
        println!("  contextx setup --all");
    } else {
        println!("  cargo run -- setup --all --dry-run");
        println!("  cargo run -- setup --all");
    }
    println!("  contextx daemon");
    println!("  contextx tui");
    if args.fix {
        println!();
        println!("Applying safe setup fixes:");
        setup::run(SetupArgs {
            all: true,
            client: None,
            dry_run: args.dry_run,
        })
        .await?;
    }
    Ok(())
}

fn check_command(command: &str) {
    match command_path(command) {
        Ok(path) => println!("command {:<18} found at {}", command, path.display()),
        Err(_) => println!("command {:<18} not found", command),
    }
}

fn command_path(command: &str) -> Result<PathBuf, which::Error> {
    which::which(command)
}

fn command_path_string(command: &str) -> Option<String> {
    command_path(command)
        .ok()
        .map(|path| path.display().to_string())
}

fn check_env(name: &str) {
    if env::var_os(name).is_some() {
        println!("env     {:<18} set", name);
    } else {
        println!("env     {:<18} not set", name);
    }
}

fn check_port(port: u16) {
    match port_available(port) {
        true => println!("port    {:<18} available", port),
        false => println!("port    {:<18} already in use", port),
    }
}

fn port_available(port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpListener::bind(addr).is_ok()
}

fn check_config(label: &str, path: Option<PathBuf>) {
    match path {
        Some(path) if path.exists() => {
            println!("config  {:<18} exists at {}", label, path.display())
        }
        Some(path) => println!(
            "config  {:<18} missing, can create {}",
            label,
            path.display()
        ),
        None => println!("config  {:<18} unknown path", label),
    }
}

fn home_path(relative: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(relative))
}

fn claude_desktop_config() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support/Claude/claude_desktop_config.json"))
    } else if cfg!(target_os = "windows") {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("Claude/claude_desktop_config.json"))
    } else {
        Some(home.join(".config/Claude/claude_desktop_config.json"))
    }
}

fn vscode_settings() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if cfg!(target_os = "windows") {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("Code/User/settings.json"))
    } else {
        Some(home.join(".config/Code/User/settings.json"))
    }
}
