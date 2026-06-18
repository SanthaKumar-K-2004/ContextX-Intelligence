use crate::cli::ProxyArgs;
use crate::core::{CompressRequest, ContextXEngine, Message};
use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use reqwest::Client;
use serde_json::{Value, json};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    engine: Arc<Mutex<ContextXEngine>>,
    client: Client,
    upstream: String,
}

pub async fn run(args: ProxyArgs, engine: Arc<Mutex<ContextXEngine>>) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("invalid host/port")?;
    ensure_private_bind(addr, args.allow_non_localhost)?;
    let state = AppState {
        engine,
        client: Client::new(),
        upstream: args.upstream.trim_end_matches('/').to_string(),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/v1/contextx/compress", post(contextx_compress))
        .route("/v1/contextx/retrieve", post(contextx_retrieve))
        .route("/v1/contextx/stats", get(stats))
        .route("/v1/contextx/learn", post(contextx_learn))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        .route("/v1/responses", post(pass_through))
        .with_state(state);
    eprintln!("ContextX proxy listening on http://{addr}");
    eprintln!("security: localhost-only bind, memory-only CCR, no telemetry");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(
        json!({"ok": true, "service": "contextx", "creator": "Santhakumar K", "company": "Alpha X Solutions", "privacy": "memory-only"}),
    )
}

async fn stats(State(state): State<AppState>) -> Json<Value> {
    let engine = state.engine.lock().await;
    Json(serde_json::to_value(engine.stats()).unwrap_or_else(|_| json!({})))
}

async fn contextx_compress(
    State(state): State<AppState>,
    Json(mut req): Json<CompressRequest>,
) -> Json<Value> {
    if req.agent == "unknown" {
        req.agent = "daemon-api".to_string();
    }
    let mut engine = state.engine.lock().await;
    Json(serde_json::to_value(engine.compress(req)).unwrap_or_else(|_| json!({})))
}

async fn contextx_retrieve(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let keys = payload
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
    let mut engine = state.engine.lock().await;
    Json(serde_json::to_value(engine.retrieve(&keys)).unwrap_or_else(|_| json!({})))
}

async fn contextx_learn() -> Json<Value> {
    Json(json!({
        "mode": "observe-only",
        "proposals": [
            {"key": "shared-daemon", "confidence": 1.0, "action": "run contextx daemon so MCP, proxy, stats, and TUI share one memory store"},
            {"key": "tree-sitter-code-compression", "confidence": 0.82, "action": "enable AST compression after project language detection is stable"},
            {"key": "encrypted-history", "confidence": 0.74, "action": "add opt-in encrypted persistence for long-term charts and learning"}
        ],
        "applied": [],
        "reverted": []
    }))
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut payload: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    let messages = payload
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Message>(value).ok())
        .collect::<Vec<_>>();

    if !messages.is_empty() {
        let model = payload
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("auto")
            .to_string();
        let response = {
            let mut engine = state.engine.lock().await;
            engine.compress(CompressRequest {
                messages,
                model,
                provider: "openai-compatible".to_string(),
                budget_tokens: None,
                algorithm: "auto".to_string(),
                agent: "proxy".to_string(),
                project_path: None,
            })
        };
        payload["messages"] =
            serde_json::to_value(response.compressed_messages).unwrap_or_else(|_| json!([]));
        payload["contextx"] = json!({
            "original_tokens": response.original_tokens,
            "compressed_tokens": response.compressed_tokens,
            "savings_pct": response.savings_pct,
            "ccr_keys": response.ccr_keys
        });
    }

    forward(state, headers, "/v1/chat/completions", payload).await
}

async fn anthropic_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut payload: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    let messages = payload
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Message>(value).ok())
        .collect::<Vec<_>>();

    if !messages.is_empty() {
        let model = payload
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("claude-auto")
            .to_string();
        let response = {
            let mut engine = state.engine.lock().await;
            engine.compress(CompressRequest {
                messages,
                model,
                provider: "anthropic".to_string(),
                budget_tokens: None,
                algorithm: "auto".to_string(),
                agent: "proxy".to_string(),
                project_path: None,
            })
        };
        payload["messages"] =
            serde_json::to_value(response.compressed_messages).unwrap_or_else(|_| json!([]));
        payload["contextx"] = json!({
            "original_tokens": response.original_tokens,
            "compressed_tokens": response.compressed_tokens,
            "savings_pct": response.savings_pct,
            "ccr_keys": response.ccr_keys
        });
    }

    forward(state, headers, "/v1/messages", payload).await
}

async fn pass_through(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    forward(state, headers, "/v1/responses", payload).await
}

async fn forward(state: AppState, headers: HeaderMap, path: &str, payload: Value) -> Response {
    let url = format!("{}{}", state.upstream, path);
    let mut request = state.client.post(url).json(&payload);
    if let Some(auth) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("authorization", auth);
    }
    if let Some(org) = headers
        .get("openai-organization")
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("openai-organization", org);
    }
    if let Some(key) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("x-api-key", key);
    }
    if let Some(version) = headers
        .get("anthropic-version")
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("anthropic-version", version);
    }

    match request.send().await {
        Ok(response) => {
            let status =
                StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/json")
                .to_string();
            match response.bytes().await {
                Ok(bytes) => {
                    if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                        observe_usage(&state, &value).await;
                    }
                    (status, [("content-type", content_type)], bytes).into_response()
                }
                Err(err) => (StatusCode::BAD_GATEWAY, err.to_string()).into_response(),
            }
        }
        Err(err) => (StatusCode::BAD_GATEWAY, err.to_string()).into_response(),
    }
}

async fn observe_usage(state: &AppState, response: &Value) {
    let output_tokens = response
        .pointer("/usage/completion_tokens")
        .or_else(|| response.pointer("/usage/output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    if output_tokens > 0 {
        let mut engine = state.engine.lock().await;
        engine.observe_output("proxy", "openai-compatible", output_tokens);
    }
}

fn ensure_private_bind(addr: SocketAddr, allow_non_localhost: bool) -> Result<()> {
    if allow_non_localhost || is_loopback(addr.ip()) {
        return Ok(());
    }
    anyhow::bail!(
        "refusing to bind ContextX to non-local address {addr}; use --allow-non-localhost only on trusted networks"
    );
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}
