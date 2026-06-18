mod cli;
mod core;
mod doctor;
mod install;
mod mcp;
mod proxy;
mod tui;
mod wrap;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use core::ContextXEngine;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Arc::new(Mutex::new(ContextXEngine::default()));

    match cli.command {
        Commands::Mcp(args) => mcp::run_stdio(args, engine).await,
        Commands::Proxy(args) => proxy::run(args, engine).await,
        Commands::Daemon(args) => proxy::run(args.into(), engine).await,
        Commands::Wrap(args) => wrap::run(args, engine).await,
        Commands::Tui(args) => tui::run(args, engine).await,
        Commands::Stats(args) => tui::print_stats(args, engine).await,
        Commands::Doctor(args) => doctor::run(args).await,
        Commands::Install(args) => install::run(args).await,
        Commands::PrintConfig(args) => install::print_config(args).await,
        Commands::VerifyClient(args) => install::verify_client(args).await,
    }
}
