use crate::cli::{StatsArgs, TuiArgs};
use crate::core::{ContextXEngine, StatsSnapshot};
use anyhow::Result;
use chrono::Local;
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

pub async fn print_status(args: StatsArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let client = Client::new();
    loop {
        let snapshot = load_snapshot(&client, &args.daemon_url, args.local, engine.clone()).await;
        println!("{}", compact_status(&snapshot));
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
        if let Ok(response) = client.get(url).send().await
            && let Ok(snapshot) = response.json::<StatsSnapshot>().await
        {
            return snapshot;
        }
    }
    engine.lock().await.stats()
}

fn render(snapshot: &StatsSnapshot) {
    print!("{}", compact_status(snapshot));
}

pub fn compact_status(snapshot: &StatsSnapshot) -> String {
    let mut output = String::new();
    output.push_str("ContextX Intelligence | Santhakumar K - Alpha X Solutions\n");
    output.push_str("Privacy: memory-only | Telemetry: off | CCR originals: RAM only\n\n");
    output.push_str("+----------+------+----------+------------+--------+--------+----------+\n");
    output.push_str("| Window   | Req  | Used     | Saved      | Save % | Burn/m | Reset    |\n");
    output.push_str("+----------+------+----------+------------+--------+--------+----------+\n");
    output.push_str(&window_row("Session", &snapshot.session, "5h est"));
    output.push_str(&window_row("Daily", &snapshot.daily, "midnight"));
    output.push_str(&window_row("Weekly", &snapshot.weekly, "7d roll"));
    output.push_str("+----------+------+----------+------------+--------+--------+----------+\n\n");
    output.push_str(&format!(
        "Daily quota: {} | remaining: {} | next reset: {}\n",
        quota(snapshot.usage.daily_quota_tokens),
        quota(snapshot.usage.daily_remaining_tokens),
        snapshot
            .usage
            .next_daily_reset_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M local")
    ));
    output.push_str(&format!(
        "Session reset estimate: {} | quota source: {}\n",
        snapshot
            .usage
            .session_reset_at
            .with_timezone(&Local)
            .format("%H:%M local"),
        snapshot.usage.quota_source
    ));
    output.push_str(&format!(
        "CCR cache: {}/{} | hits {} | misses {}\n\n",
        snapshot.cache.ccr_items,
        snapshot.cache.ccr_limit,
        snapshot.cache.cache_hits,
        snapshot.cache.cache_misses
    ));

    output.push_str("Top agents\n");
    output.push_str("+--------------------+------+----------+----------+----------+\n");
    output.push_str("| Agent              | Req  | Original | Sent     | Output   |\n");
    output.push_str("+--------------------+------+----------+----------+----------+\n");
    let mut agents = snapshot.by_agent.iter().collect::<Vec<_>>();
    agents
        .sort_by_key(|(_, stats)| std::cmp::Reverse(stats.compressed_tokens + stats.output_tokens));
    for (agent, stats) in agents.into_iter().take(5) {
        output.push_str(&format!(
            "| {:<18.18} | {:>4} | {:>8} | {:>8} | {:>8} |\n",
            agent,
            stats.requests,
            stats.original_tokens,
            stats.compressed_tokens,
            stats.output_tokens
        ));
    }
    if snapshot.by_agent.is_empty() {
        output.push_str("| No activity yet. Send traffic through MCP, proxy, or wrap.   |\n");
    }
    output.push_str("+--------------------+------+----------+----------+----------+\n\n");

    output.push_str("Recent events\n");
    output.push_str("+----------+--------------+--------------+--------+----------+\n");
    output.push_str("| Time     | Agent        | Kind         | Save % | Hash     |\n");
    output.push_str("+----------+--------------+--------------+--------+----------+\n");
    for event in snapshot.recent_events.iter().rev().take(6) {
        output.push_str(&format!(
            "| {} | {:<12.12} | {:<12.12} | {:>6.1} | {:<8.8} |\n",
            event.ts.format("%H:%M:%S"),
            event.agent,
            event.kind,
            event.savings_pct,
            event.request_hash
        ));
    }
    if snapshot.recent_events.is_empty() {
        output.push_str("| No events yet.                                              |\n");
    }
    output.push_str("+----------+--------------+--------------+--------+----------+\n");
    output
}

fn window_row(label: &str, stats: &crate::core::WindowStats, reset: &str) -> String {
    format!(
        "| {:<8} | {:>4} | {:>8} | {:>10} | {:>6.1} | {:>6.1} | {:<8} |\n",
        label,
        stats.requests,
        stats.used_tokens,
        stats.saved_tokens,
        stats.savings_pct,
        stats.burn_tokens_per_minute,
        reset
    )
}

fn quota(value: Option<usize>) -> String {
    value
        .map(|tokens| tokens.to_string())
        .unwrap_or_else(|| "not set".to_string())
}

#[allow(dead_code)]
fn render_legacy(snapshot: &StatsSnapshot) {
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
