use crate::cli::WrapArgs;
use anyhow::{Context, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

pub async fn run(args: WrapArgs, _engine: Arc<Mutex<crate::core::ContextXEngine>>) -> Result<()> {
    let started = std::time::Instant::now();
    eprintln!(
        "contextx wrap: starting `{}` with process tracking",
        args.command
    );
    eprintln!(
        "contextx wrap: token usage is only exact when traffic flows through ContextX MCP or proxy"
    );
    let status = Command::new(&args.command)
        .args(&args.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to run {}", args.command))?;
    let elapsed = started.elapsed().as_secs();
    eprintln!("contextx wrap: command exited with {status} after {elapsed}s");
    Ok(())
}
