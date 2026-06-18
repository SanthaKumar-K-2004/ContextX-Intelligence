use crate::cli::{ClientArgs, InstallArgs, SetupArgs};
use crate::install;
use anyhow::{Context, Result, anyhow};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(args: SetupArgs) -> Result<()> {
    println!("ContextX Setup");
    println!("==============");
    println!("Santhakumar K • Alpha X Solutions");
    println!("privacy: no telemetry, no API key copying, localhost-only services");
    println!();

    let stable_binary = stable_binary_path()?;
    let current_binary = env::current_exe().context("detect current contextx executable")?;
    ensure_stable_binary(&current_binary, &stable_binary, args.dry_run)?;
    ensure_path(args.dry_run)?;

    let install_args = InstallArgs {
        all: args.all,
        client: args
            .client
            .clone()
            .or_else(|| (!args.all).then(|| "claude-desktop".to_string())),
        dry_run: args.dry_run,
    };

    println!();
    println!("Tool configuration:");
    warn_missing_tools(install_args.client.as_deref(), install_args.all);
    install::run_with_binary(install_args, &stable_binary.display().to_string()).await?;

    if !args.dry_run {
        println!();
        println!("Verification:");
        verify_selected(args.client.as_deref(), args.all).await?;
    }

    println!();
    println!("Next commands:");
    println!("  contextx daemon");
    println!("  contextx tui");
    println!("  contextx stats --watch");
    println!();
    println!("If this terminal still says `contextx: command not found`, restart it or run:");
    println!("  export PATH=\"$HOME/.local/bin:$PATH\"");
    Ok(())
}

fn stable_binary_path() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        return env::current_exe().context("detect current contextx executable");
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not find home directory"))?;
    Ok(home.join(".local/bin/contextx"))
}

fn ensure_stable_binary(current: &Path, stable: &Path, dry_run: bool) -> Result<()> {
    if current == stable {
        println!("binary: already installed at {}", stable.display());
        return Ok(());
    }
    if dry_run {
        println!(
            "binary: would copy {} to {}",
            current.display(),
            stable.display()
        );
        return Ok(());
    }
    if let Some(parent) = stable.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(current, stable)
        .with_context(|| format!("copy {} to {}", current.display(), stable.display()))?;
    make_executable(stable)?;
    println!("binary: installed at {}", stable.display());
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn ensure_path(dry_run: bool) -> Result<()> {
    if cfg!(target_os = "windows") {
        println!("path: Windows automatic PATH editing is not enabled yet");
        return Ok(());
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not find home directory"))?;
    let local_bin = home.join(".local/bin");
    let path_env = env::var_os("PATH").unwrap_or_default();
    if env::split_paths(&path_env).any(|entry| entry == local_bin) {
        println!("path: ~/.local/bin already available");
        return Ok(());
    }
    let Some(shell_config) = shell_config_path(&home) else {
        println!("path: could not detect shell config; add this manually:");
        println!("  export PATH=\"$HOME/.local/bin:$PATH\"");
        return Ok(());
    };
    add_path_to_shell_config(&shell_config, dry_run)
}

fn shell_config_path(home: &Path) -> Option<PathBuf> {
    let shell = env::var("SHELL").unwrap_or_default();
    shell_config_path_for(home, &shell)
}

fn shell_config_path_for(home: &Path, shell: &str) -> Option<PathBuf> {
    if shell.ends_with("fish") {
        Some(home.join(".config/fish/config.fish"))
    } else if shell.ends_with("bash") {
        Some(home.join(".bashrc"))
    } else if shell.ends_with("zsh") || shell.is_empty() {
        Some(home.join(".zshrc"))
    } else {
        None
    }
}

fn add_path_to_shell_config(path: &Path, dry_run: bool) -> Result<()> {
    let line = if path.file_name().and_then(|name| name.to_str()) == Some("config.fish") {
        "fish_add_path $HOME/.local/bin"
    } else {
        "export PATH=\"$HOME/.local/bin:$PATH\""
    };
    let content = fs::read_to_string(path).unwrap_or_default();
    if path_line_exists(&content) {
        println!("path: shell config already contains ~/.local/bin");
        return Ok(());
    }
    if dry_run {
        println!("path: would add ~/.local/bin to {}", path.display());
        if path.exists() {
            println!("path: would create backup {}", backup_path(path).display());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        fs::copy(path, backup_path(path))?;
    }
    let prefix = if content.ends_with('\n') || content.is_empty() {
        ""
    } else {
        "\n"
    };
    fs::write(path, format!("{content}{prefix}{line}\n"))?;
    println!("path: added ~/.local/bin to {}", path.display());
    Ok(())
}

fn path_line_exists(content: &str) -> bool {
    content.contains("$HOME/.local/bin")
        || content.contains("~/.local/bin")
        || content.contains(".local/bin")
}

fn warn_missing_tools(client: Option<&str>, all: bool) {
    for target in install::selected_targets(client) {
        if all || client.is_some() {
            let installed = match target.client {
                "cursor" => which::which("cursor").is_ok(),
                "vscode" => which::which("code").is_ok(),
                "zed" => which::which("zed").is_ok(),
                "claude-desktop" => target.path.exists() || which::which("claude").is_ok(),
                _ => false,
            };
            if !installed {
                println!(
                    "{} not detected. ContextX config can still be created, but install the app first to use it.",
                    target.label
                );
            }
        }
    }
}

async fn verify_selected(client: Option<&str>, all: bool) -> Result<()> {
    let clients: Vec<String> = if all {
        install::selected_targets(None)
            .into_iter()
            .map(|target| target.client.to_string())
            .collect()
    } else {
        vec![client.unwrap_or("claude-desktop").to_string()]
    };
    for client in clients {
        install::verify_client(ClientArgs { client }).await?;
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("shellrc");
    path.with_file_name(format!("{file_name}.contextx-backup-{stamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_path_line() {
        assert!(path_line_exists("export PATH=\"$HOME/.local/bin:$PATH\""));
        assert!(path_line_exists("fish_add_path $HOME/.local/bin"));
        assert!(!path_line_exists("export PATH=\"/usr/bin:$PATH\""));
    }

    #[test]
    fn backup_name_contains_original_file() {
        let backup = backup_path(Path::new("/tmp/.zshrc"));
        let name = backup.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".zshrc.contextx-backup-"));
    }

    #[test]
    fn path_dry_run_writes_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let rc = temp.path().join(".zshrc");
        fs::write(&rc, "# existing\n").unwrap();

        add_path_to_shell_config(&rc, true).unwrap();

        assert_eq!(fs::read_to_string(&rc).unwrap(), "# existing\n");
    }

    #[test]
    fn path_line_is_not_duplicated() {
        let temp = tempfile::tempdir().unwrap();
        let rc = temp.path().join(".zshrc");
        fs::write(&rc, "export PATH=\"$HOME/.local/bin:$PATH\"\n").unwrap();

        add_path_to_shell_config(&rc, false).unwrap();

        assert_eq!(
            fs::read_to_string(&rc).unwrap(),
            "export PATH=\"$HOME/.local/bin:$PATH\"\n"
        );
    }

    #[test]
    fn shell_config_has_zsh_fallback() {
        let home = Path::new("/home/example");
        assert_eq!(
            shell_config_path_for(home, "")
                .unwrap()
                .file_name()
                .unwrap(),
            ".zshrc"
        );
        assert_eq!(
            shell_config_path_for(home, "/bin/bash")
                .unwrap()
                .file_name()
                .unwrap(),
            ".bashrc"
        );
        assert_eq!(
            shell_config_path_for(home, "/usr/bin/fish")
                .unwrap()
                .file_name()
                .unwrap(),
            "config.fish"
        );
    }
}
