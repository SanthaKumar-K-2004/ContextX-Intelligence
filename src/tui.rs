use crate::cli::{StatsArgs, TuiArgs};
use crate::core::{ContextXEngine, StatsSnapshot};
use anyhow::Result;
use reqwest::Client;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

pub async fn run(args: TuiArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let client = Client::new();
    loop {
        let snapshot = load_snapshot(&client, &args.daemon_url, args.local, engine.clone()).await;
        print!("\x1B[2J\x1B[H");
        render(&snapshot);
        io::stdout().flush()?;
        sleep(Duration::from_secs(args.refresh_seconds)).await;
    }
}

pub async fn print_stats(args: StatsArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let client = Client::new();
    loop {
        let snapshot = load_snapshot(&client, &args.daemon_url, args.local, engine.clone()).await;
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        if !args.watch {
            return Ok(());
        }
        sleep(Duration::from_secs(args.refresh_seconds)).await;
    }
}

async fn load_snapshot(
    client: &Client,
    daemon_url: &str,
    local: bool,
    engine: Arc<Mutex<ContextXEngine>>,
) -> StatsSnapshot {
    if !local {
        let url = format!("{}/v1/contextx/stats", daemon_url.trim_end_matches('/'));
        if let Ok(response) = client.get(url).send().await {
            if let Ok(snapshot) = response.json::<StatsSnapshot>().await {
                return snapshot;
            }
        }
    }
    engine.lock().await.stats()
}

fn render(snapshot: &StatsSnapshot) {
    println!("ContextX Intelligence");
    println!("Santhakumar K • Alpha X Solutions");
    println!("Privacy: memory-only | Telemetry: off | CCR originals: RAM only");
    println!();
    println!(
        "Session  requests {:>4} | original {:>8} | compressed {:>8} | output {:>8} | saved {:>5.1}% | burn {:>6.1}/min",
        snapshot.session.requests,
        snapshot.session.original_tokens,
        snapshot.session.compressed_tokens,
        snapshot.session.output_tokens,
        snapshot.session.savings_pct,
        snapshot.session.burn_tokens_per_minute
    );
    println!(
        "Weekly   requests {:>4} | original {:>8} | compressed {:>8} | output {:>8} | saved {:>5.1}% | burn {:>6.1}/min",
        snapshot.weekly.requests,
        snapshot.weekly.original_tokens,
        snapshot.weekly.compressed_tokens,
        snapshot.weekly.output_tokens,
        snapshot.weekly.savings_pct,
        snapshot.weekly.burn_tokens_per_minute
    );
    println!();
    println!(
        "CCR cache: {} / {} items | hits {} | misses {}",
        snapshot.cache.ccr_items,
        snapshot.cache.ccr_limit,
        snapshot.cache.cache_hits,
        snapshot.cache.cache_misses
    );
    println!();
    println!("Agents");
    for (agent, stats) in &snapshot.by_agent {
        println!(
            "  {:<18} requests {:>4} | original {:>8} | compressed {:>8} | output {:>8}",
            agent,
            stats.requests,
            stats.original_tokens,
            stats.compressed_tokens,
            stats.output_tokens
        );
    }
    if snapshot.by_agent.is_empty() {
        println!("  No activity yet. Send traffic through MCP, proxy, or wrap.");
    }
    println!();
    println!("Recent Events");
    for event in snapshot.recent_events.iter().rev().take(8) {
        println!(
            "  {} {:<12} {:<12} saved {:>5.1}% hash {}",
            event.ts.format("%H:%M:%S"),
            event.agent,
            event.kind,
            event.savings_pct,
            event.request_hash
        );
    }
}
