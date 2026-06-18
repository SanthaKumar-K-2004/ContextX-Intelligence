use crate::cli::WrapArgs;
use crate::core::ContextXEngine;
use anyhow::{Context, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

pub async fn run(args: WrapArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let started = std::time::Instant::now();
    eprintln!(
        "contextx wrap: starting `{}` with memory-only tracking",
        args.command
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
    {
        let mut engine = engine.lock().await;
        engine.observe_output(
            &format!("wrap:{}", args.command),
            "local-cli",
            elapsed as usize,
        );
    }
    eprintln!("contextx wrap: command exited with {status}");
    Ok(())
}
