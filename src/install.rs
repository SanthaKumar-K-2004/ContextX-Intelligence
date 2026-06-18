use crate::cli::{ClientArgs, InstallArgs};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(args: InstallArgs) -> Result<()> {
    if !args.all && args.client.is_none() {
        println!(
            "Nothing selected. Use `contextx install --all` or `contextx install --client claude-desktop`."
        );
        return Ok(());
    }
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(str::to_string))
        .unwrap_or_else(|| "contextx".to_string());
    let targets = selected_targets(args.client.as_deref());
    for target in targets {
        if args.dry_run {
            println!(
                "would configure {} at {}{}",
                target.label,
                target.path.display(),
                if target.path.exists() {
                    " and create a backup first"
                } else {
                    ""
                }
            );
        } else {
            write_mcp_config(&target, &binary)
                .with_context(|| format!("configure {}", target.label))?;
            println!("configured {} at {}", target.label, target.path.display());
        }
    }
    println!();
    println!("Proxy setup for OpenAI-compatible tools:");
    println!("  export OPENAI_BASE_URL=http://127.0.0.1:8787/v1");
    println!("  contextx proxy --port 8787");
    Ok(())
}

pub async fn print_config(args: ClientArgs) -> Result<()> {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(str::to_string))
        .unwrap_or_else(|| "contextx".to_string());
    println!(
        "{}",
        serde_json::to_string_pretty(&config_for_client(&args.client, &binary))?
    );
    Ok(())
}

pub async fn verify_client(args: ClientArgs) -> Result<()> {
    let targets = selected_targets(Some(&args.client));
    if targets.is_empty() {
        println!("unsupported client `{}`", args.client);
        return Ok(());
    }
    for target in targets {
        if !target.path.exists() {
            println!(
                "{}: missing config at {}",
                target.label,
                target.path.display()
            );
            continue;
        }
        let content = fs::read_to_string(&target.path)?;
        let value: Value = serde_json::from_str(&content)?;
        if contains_contextx(&value) {
            println!(
                "{}: ContextX configured at {}",
                target.label,
                target.path.display()
            );
        } else {
            println!(
                "{}: config exists but ContextX is not installed",
                target.label
            );
        }
    }
    Ok(())
}

struct Target {
    label: &'static str,
    client: &'static str,
    path: PathBuf,
}

fn selected_targets(client: Option<&str>) -> Vec<Target> {
    install_targets()
        .into_iter()
        .filter(|target| client.is_none_or(|client| target.client == normalize_client(client)))
        .collect()
}

fn install_targets() -> Vec<Target> {
    let mut targets = Vec::new();
    if let Some(home) = dirs::home_dir() {
        targets.push(Target {
            label: "Cursor MCP",
            client: "cursor",
            path: home.join(".cursor/mcp.json"),
        });
        targets.push(Target {
            label: "VS Code MCP",
            client: "vscode",
            path: home.join(".config/Code/User/mcp.json"),
        });
        targets.push(Target {
            label: "Zed MCP",
            client: "zed",
            path: home.join(".config/zed/settings.json"),
        });
        if cfg!(target_os = "macos") {
            targets.push(Target {
                label: "Claude Desktop",
                client: "claude-desktop",
                path: home.join("Library/Application Support/Claude/claude_desktop_config.json"),
            });
        } else if cfg!(target_os = "windows") {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                targets.push(Target {
                    label: "Claude Desktop",
                    client: "claude-desktop",
                    path: PathBuf::from(appdata).join("Claude/claude_desktop_config.json"),
                });
            }
        } else {
            targets.push(Target {
                label: "Claude Desktop",
                client: "claude-desktop",
                path: home.join(".config/Claude/claude_desktop_config.json"),
            });
        }
    }
    targets
}

fn write_mcp_config(target: &Target, binary: &str) -> Result<()> {
    let path = &target.path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let backup = backup_path(path);
        fs::copy(path, &backup)?;
    }
    let existing = if path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(path)?)?
    } else {
        json!({})
    };
    let mut root = existing.as_object().cloned().unwrap_or_default();
    let servers_key = if target.client == "vscode" {
        "servers"
    } else {
        "mcpServers"
    };
    let mut servers = root
        .remove(servers_key)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    servers.insert(
        "contextx".to_string(),
        json!({
            "command": binary,
            "args": ["mcp"]
        }),
    );
    root.insert(servers_key.to_string(), Value::Object(servers));
    fs::write(path, serde_json::to_string_pretty(&Value::Object(root))?)?;
    Ok(())
}

fn config_for_client(client: &str, binary: &str) -> Value {
    let server = json!({
        "command": binary,
        "args": ["mcp"]
    });
    match normalize_client(client) {
        "vscode" => json!({"servers": {"contextx": server}}),
        _ => json!({"mcpServers": {"contextx": server}}),
    }
}

fn normalize_client(client: &str) -> &'static str {
    match client {
        "claude" | "claude_desktop" | "claude-desktop" => "claude-desktop",
        "cursor" => "cursor",
        "code" | "vscode" | "vs-code" => "vscode",
        "zed" => "zed",
        _ => "unsupported",
    }
}

fn contains_contextx(value: &Value) -> bool {
    value
        .get("mcpServers")
        .and_then(Value::as_object)
        .is_some_and(|servers| servers.contains_key("contextx"))
        || value
            .get("servers")
            .and_then(Value::as_object)
            .is_some_and(|servers| servers.contains_key("contextx"))
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    path.with_file_name(format!("{file_name}.contextx-backup-{stamp}"))
}
