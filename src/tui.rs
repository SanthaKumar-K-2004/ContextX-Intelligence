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
        print!("\x1B[?25l\x1B[2J\x1B[H");
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
        if args.watch {
            print!("\x1B[?25l\x1B[2J\x1B[H");
            print!("{}", compact_status(&snapshot));
            io::stdout().flush()?;
        } else {
            println!("{}", compact_status(&snapshot));
        }
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
    format!(
        "ContextX usage\n\
Daily: used {} | quota {} | remaining {} | reset {}\n\
Session: {} req | used {} | saved {} | {:.1} tok/min\n\
Weekly: {} req / {} used / {} saved / {:.1}% saved\n\
CCR: {}/{} items / {} hits / {} misses | memory-only, telemetry off\n{}",
        tokens(snapshot.usage.used_tokens),
        quota(snapshot.usage.daily_quota_tokens),
        quota(snapshot.usage.daily_remaining_tokens),
        snapshot
            .usage
            .next_daily_reset_at
            .with_timezone(&Local)
            .format("%H:%M local"),
        snapshot.session.requests,
        tokens(snapshot.session.used_tokens),
        tokens(snapshot.session.saved_tokens),
        snapshot.session.burn_tokens_per_minute,
        snapshot.weekly.requests,
        tokens(snapshot.weekly.used_tokens),
        tokens(snapshot.weekly.saved_tokens),
        snapshot.weekly.savings_pct,
        snapshot.cache.ccr_items,
        snapshot.cache.ccr_limit,
        snapshot.cache.cache_hits,
        snapshot.cache.cache_misses,
        activity_line(snapshot)
    )
}

pub fn desktop_status(snapshot: &StatsSnapshot) -> String {
    format!(
        "ContextX usage\nDaily: used {} | quota {} | remaining {}\nSaved: {} | req {} | CCR {}/{}\nReset: {} | {}",
        tokens(snapshot.usage.used_tokens),
        quota(snapshot.usage.daily_quota_tokens),
        quota(snapshot.usage.daily_remaining_tokens),
        tokens(snapshot.usage.saved_tokens),
        snapshot.daily.requests,
        snapshot.cache.ccr_items,
        snapshot.cache.ccr_limit,
        snapshot
            .usage
            .next_daily_reset_at
            .with_timezone(&Local)
            .format("%H:%M local"),
        short_activity(snapshot)
    )
}

fn activity_line(snapshot: &StatsSnapshot) -> String {
    if let Some(event) = snapshot.recent_events.last() {
        return format!(
            "Last: {} {} saved {:.1}% at {}\n",
            event.agent,
            event.kind,
            event.savings_pct,
            event.ts.with_timezone(&Local).format("%H:%M:%S"),
        );
    }
    "Last: no activity yet. Send traffic through MCP, proxy, or wrap.\n".to_string()
}

fn short_activity(snapshot: &StatsSnapshot) -> String {
    snapshot
        .recent_events
        .last()
        .map(|event| format!("last {} saved {:.1}%", event.agent, event.savings_pct))
        .unwrap_or_else(|| "no activity yet".to_string())
}

fn tokens(value: usize) -> String {
    format!("{} tok", short_number(value))
}

fn quota(value: Option<usize>) -> String {
    value.map(tokens).unwrap_or_else(|| "not set".to_string())
}

fn short_number(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 10_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_status_stays_small() {
        let snapshot = ContextXEngine::default().stats();
        let status = desktop_status(&snapshot);

        assert!(status.lines().count() <= 4);
        assert!(status.contains("Daily:"));
        assert!(status.contains("remaining"));
        assert!(status.contains("CCR"));
    }

    #[test]
    fn compact_status_has_no_large_agent_or_event_tables() {
        let snapshot = ContextXEngine::default().stats();
        let status = compact_status(&snapshot);

        assert!(!status.contains("Top agents"));
        assert!(!status.contains("Recent events"));
        assert!(status.lines().count() <= 6);
    }
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
