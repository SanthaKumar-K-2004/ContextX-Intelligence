use crate::cli::McpArgs;
use crate::core::{CompressRequest, ContextXEngine, StatsSnapshot};
use crate::tui;
use anyhow::Result;
use reqwest::Client;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub async fn run_stdio(args: McpArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = io::stdout();
    let daemon = DaemonClient::new(args.daemon_url, args.local);

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                write_response(&mut stdout, json!({"jsonrpc":"2.0","error":{"code":-32700,"message":err.to_string()},"id":null})).await?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "contextx", "version": env!("CARGO_PKG_VERSION")}
                }
            }),
            "notifications/initialized" => continue,
            "tools/list" => json!({"jsonrpc":"2.0","id":id,"result":{"tools": tools()}}),
            "tools/call" => {
                handle_tool_call(
                    id,
                    request.get("params").cloned().unwrap_or_default(),
                    &daemon,
                    engine.clone(),
                )
                .await
            }
            _ => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("unknown method {method}")}})
            }
        };
        write_response(&mut stdout, response).await?;
    }
    Ok(())
}

async fn handle_tool_call(
    id: Value,
    params: Value,
    daemon: &DaemonClient,
    engine: Arc<Mutex<ContextXEngine>>,
) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let result = match name {
        "contextx_compress" => {
            let req: CompressRequest =
                serde_json::from_value(args).unwrap_or_else(|_| CompressRequest {
                    messages: Vec::new(),
                    model: "auto".to_string(),
                    provider: "auto".to_string(),
                    budget_tokens: None,
                    algorithm: "auto".to_string(),
                    agent: "mcp".to_string(),
                    project_path: None,
                });
            if let Some(value) = daemon.post_json("/v1/contextx/compress", &req).await {
                return tool_result(id, value);
            }
            let mut engine = engine.lock().await;
            serde_json::to_value(engine.compress(req)).unwrap_or_else(|_| json!({}))
        }
        "contextx_retrieve" => {
            let keys = args
                .get("ccr_keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Some(value) = daemon
                .post_json("/v1/contextx/retrieve", &json!({"ccr_keys": keys}))
                .await
            {
                return tool_result(id, value);
            }
            let mut engine = engine.lock().await;
            serde_json::to_value(engine.retrieve(&keys)).unwrap_or_else(|_| json!({}))
        }
        "contextx_stats" => {
            if let Some(value) = daemon.get_json("/v1/contextx/stats").await {
                return tool_result(id, value);
            }
            let engine = engine.lock().await;
            serde_json::to_value(engine.stats()).unwrap_or_else(|_| json!({}))
        }
        "contextx_status" => {
            if let Some(value) = daemon.get_json("/v1/contextx/stats").await {
                let parsed = serde_json::from_value::<StatsSnapshot>(value.clone());
                let text = parsed
                    .as_ref()
                    .map(tui::desktop_status)
                    .unwrap_or_else(|_| serde_json::to_string_pretty(&value).unwrap_or_default());
                let structured = parsed.as_ref().map(status_summary_json).unwrap_or(value);
                return tool_text_result(id, text, structured);
            }
            let engine = engine.lock().await;
            let snapshot = engine.stats();
            let value = status_summary_json(&snapshot);
            return tool_text_result(id, tui::desktop_status(&snapshot), value);
        }
        "contextx_learn" => {
            if let Some(value) = daemon.post_json("/v1/contextx/learn", &args).await {
                return tool_result(id, value);
            }
            json!({
                "proposals": [
                    {"key": "observe-only", "confidence": 1.0, "savings_delta": "collect more local observations before auto-tuning"}
                ],
                "applied": [],
                "reverted": []
            })
        }
        _ => {
            return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":format!("unknown tool {name}")}});
        }
    };
    tool_result(id, result)
}

fn tool_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default()}],
            "structuredContent": result
        }
    })
}

fn tool_text_result(id: Value, text: String, structured_content: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": text}],
            "structuredContent": structured_content
        }
    })
}

fn status_summary_json(snapshot: &StatsSnapshot) -> Value {
    json!({
        "daily": {
            "used_tokens": snapshot.usage.used_tokens,
            "quota_tokens": snapshot.usage.daily_quota_tokens,
            "remaining_tokens": snapshot.usage.daily_remaining_tokens,
            "saved_tokens": snapshot.usage.saved_tokens,
            "requests": snapshot.daily.requests,
            "reset_at": snapshot.usage.next_daily_reset_at,
        },
        "session": {
            "used_tokens": snapshot.session.used_tokens,
            "saved_tokens": snapshot.session.saved_tokens,
            "burn_tokens_per_minute": snapshot.session.burn_tokens_per_minute,
            "reset_at": snapshot.usage.session_reset_at,
        },
        "cache": {
            "ccr_items": snapshot.cache.ccr_items,
            "ccr_limit": snapshot.cache.ccr_limit,
        },
        "last_event": snapshot.recent_events.last().map(|event| json!({
            "agent": event.agent,
            "kind": event.kind,
            "savings_pct": event.savings_pct,
            "time": event.ts,
        })),
        "privacy": "memory-only",
        "telemetry": "off"
    })
}

struct DaemonClient {
    base_url: String,
    local: bool,
    client: Client,
}

impl DaemonClient {
    fn new(base_url: String, local: bool) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            local,
            client: Client::new(),
        }
    }

    async fn get_json(&self, path: &str) -> Option<Value> {
        if self.local {
            return None;
        }
        self.client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .ok()?
            .json::<Value>()
            .await
            .ok()
    }

    async fn post_json<T: serde::Serialize + ?Sized>(&self, path: &str, body: &T) -> Option<Value> {
        if self.local {
            return None;
        }
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(body)
            .send()
            .await
            .ok()?
            .json::<Value>()
            .await
            .ok()
    }
}

async fn write_response(stdout: &mut io::Stdout, value: Value) -> Result<()> {
    stdout
        .write_all(serde_json::to_string(&value)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

fn tools() -> Value {
    json!([
        {
            "name": "contextx_compress",
            "description": "Compress chat messages locally and return reversible CCR retrieval keys.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "messages": {"type": "array"},
                    "model": {"type": "string"},
                    "provider": {"type": "string"},
                    "budget_tokens": {"type": "integer"},
                    "algorithm": {"type": "string", "enum": ["auto", "json", "code", "prose", "passthrough"]},
                    "agent": {"type": "string"},
                    "project_path": {"type": "string"}
                },
                "required": ["messages"]
            }
        },
        {
            "name": "contextx_retrieve",
            "description": "Retrieve original content from the in-memory CCR cache.",
            "inputSchema": {
                "type": "object",
                "properties": {"ccr_keys": {"type": "array", "items": {"type": "string"}}},
                "required": ["ccr_keys"]
            }
        },
        {
            "name": "contextx_stats",
            "description": "Return memory-only usage, compression, cache, and learning status.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "contextx_status",
            "description": "Return a tiny human-readable usage card for Claude Desktop, Cursor, and other MCP clients.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "contextx_learn",
            "description": "Return observe-only learning proposals. Auto-tuning is disabled in v1.",
            "inputSchema": {"type": "object", "properties": {"apply": {"type": "boolean"}}}
        }
    ])
}
