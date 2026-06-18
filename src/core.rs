use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const DEFAULT_CCR_TTL_MINUTES: i64 = 24 * 60;
const DEFAULT_CCR_LIMIT: usize = 512;
const EVENT_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressRequest {
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default = "auto")]
    pub model: String,
    #[serde(default = "auto")]
    pub provider: String,
    #[serde(default)]
    pub budget_tokens: Option<usize>,
    #[serde(default = "auto")]
    pub algorithm: String,
    #[serde(default = "unknown_agent")]
    pub agent: String,
    #[serde(default)]
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResponse {
    pub compressed_messages: Vec<Message>,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub savings_pct: f64,
    pub algorithm_used: String,
    pub ccr_keys: Vec<String>,
    pub latency_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveResponse {
    pub contents: Vec<RetrievedContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContent {
    pub key: String,
    pub content: String,
    pub content_type: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub session: WindowStats,
    pub weekly: WindowStats,
    pub by_agent: HashMap<String, AgentStats>,
    pub by_provider: HashMap<String, AgentStats>,
    pub cache: CacheStats,
    pub recent_events: Vec<Event>,
    pub learning: LearningStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub output_tokens: usize,
    pub requests: usize,
    pub savings_pct: f64,
    pub burn_tokens_per_minute: f64,
    pub estimated_minutes_left: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub output_tokens: usize,
    pub requests: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub ccr_items: usize,
    pub ccr_limit: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStatus {
    pub mode: String,
    pub proposals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: DateTime<Utc>,
    pub kind: String,
    pub agent: String,
    pub provider: String,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub savings_pct: f64,
    pub request_hash: String,
}

#[derive(Debug, Clone)]
struct CcrEntry {
    content: String,
    content_type: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ContextXEngine {
    ccr: HashMap<String, CcrEntry>,
    ccr_order: VecDeque<String>,
    events: VecDeque<Event>,
    session_start: DateTime<Utc>,
    cache_hits: usize,
    cache_misses: usize,
}

impl Default for ContextXEngine {
    fn default() -> Self {
        Self {
            ccr: HashMap::new(),
            ccr_order: VecDeque::new(),
            events: VecDeque::new(),
            session_start: Utc::now(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }
}

impl ContextXEngine {
    pub fn compress(&mut self, req: CompressRequest) -> CompressResponse {
        let started = std::time::Instant::now();
        self.evict_expired();

        let original_tokens: usize = req.messages.iter().map(message_tokens).sum();
        let mut ccr_keys = Vec::new();
        let mut algorithms = Vec::new();
        let compressed_messages = req
            .messages
            .iter()
            .map(|message| {
                let (content, key, algorithm) =
                    self.compress_content(&message.content, &req.algorithm);
                if let Some(key) = key {
                    ccr_keys.push(key);
                }
                algorithms.push(algorithm);
                Message {
                    role: message.role.clone(),
                    content,
                }
            })
            .collect::<Vec<_>>();
        let compressed_tokens: usize = compressed_messages.iter().map(message_tokens).sum();
        let savings_pct = savings_pct(original_tokens, compressed_tokens);
        let algorithm_used = summarize_algorithms(&algorithms);

        let event = Event {
            ts: Utc::now(),
            kind: "compression".to_string(),
            agent: req.agent.clone(),
            provider: req.provider.clone(),
            original_tokens,
            compressed_tokens,
            savings_pct,
            request_hash: hash_json(&req.messages),
        };
        self.push_event(event);

        CompressResponse {
            compressed_messages,
            original_tokens,
            compressed_tokens,
            savings_pct,
            algorithm_used,
            ccr_keys,
            latency_ms: started.elapsed().as_millis(),
        }
    }

    pub fn retrieve(&mut self, keys: &[String]) -> RetrieveResponse {
        self.evict_expired();
        let mut contents = Vec::new();
        for key in keys {
            if let Some(entry) = self.ccr.get(key) {
                self.cache_hits += 1;
                contents.push(RetrievedContent {
                    key: key.clone(),
                    content: entry.content.clone(),
                    content_type: entry.content_type.clone(),
                    expires_at: entry.expires_at,
                });
            } else {
                self.cache_misses += 1;
            }
        }
        RetrieveResponse { contents }
    }

    pub fn observe_output(&mut self, agent: &str, provider: &str, output_tokens: usize) {
        let event = Event {
            ts: Utc::now(),
            kind: "output".to_string(),
            agent: agent.to_string(),
            provider: provider.to_string(),
            original_tokens: 0,
            compressed_tokens: output_tokens,
            savings_pct: 0.0,
            request_hash: Uuid::new_v4().to_string(),
        };
        self.push_event(event);
    }

    pub fn stats(&self) -> StatsSnapshot {
        let now = Utc::now();
        let session_events = self
            .events
            .iter()
            .filter(|event| event.ts >= self.session_start)
            .cloned()
            .collect::<Vec<_>>();
        let weekly_events = self
            .events
            .iter()
            .filter(|event| event.ts >= now - Duration::days(7))
            .cloned()
            .collect::<Vec<_>>();
        let mut by_agent = HashMap::new();
        let mut by_provider = HashMap::new();
        for event in &weekly_events {
            add_agent_stats(by_agent.entry(event.agent.clone()).or_default(), event);
            add_agent_stats(
                by_provider.entry(event.provider.clone()).or_default(),
                event,
            );
        }

        let mut recent_events = self
            .events
            .iter()
            .rev()
            .take(20)
            .cloned()
            .collect::<Vec<_>>();
        recent_events.reverse();

        StatsSnapshot {
            session: window_stats(&session_events, self.session_start, Some(5 * 60)),
            weekly: window_stats(
                &weekly_events,
                weekly_events.first().map(|event| event.ts).unwrap_or(now),
                Some(7 * 24 * 60),
            ),
            by_agent,
            by_provider,
            cache: CacheStats {
                ccr_items: self.ccr.len(),
                ccr_limit: DEFAULT_CCR_LIMIT,
                cache_hits: self.cache_hits,
                cache_misses: self.cache_misses,
            },
            recent_events,
            learning: LearningStatus {
                mode: "observe-only".to_string(),
                proposals: vec![
                    "Enable persistence later to compare compression quality across sessions."
                        .to_string(),
                    "Promote repeated successful algorithm choices after enough observations."
                        .to_string(),
                ],
            },
        }
    }

    fn compress_content(
        &mut self,
        value: &Value,
        requested: &str,
    ) -> (Value, Option<String>, String) {
        match value {
            Value::String(text) => {
                let algorithm = choose_algorithm(text, requested);
                let compressed = match algorithm.as_str() {
                    "json" => compress_json_text(text),
                    "code" => compress_code_text(text),
                    "prose" => compress_prose_text(text),
                    _ => text.clone(),
                };
                if compressed == *text {
                    (Value::String(compressed), None, "passthrough".to_string())
                } else {
                    let original_tokens = estimate_tokens(text);
                    let projected_tokens = estimate_tokens(&compressed) + 14;
                    if requested == "auto" && projected_tokens >= original_tokens {
                        return (Value::String(text.clone()), None, "passthrough".to_string());
                    }
                    let key = self.store_ccr(text, &algorithm);
                    (
                        Value::String(format!(
                            "{}\n\n[ContextX CCR: original retrievable with key {}]",
                            compressed, key
                        )),
                        Some(key),
                        algorithm,
                    )
                }
            }
            other => {
                let text = serde_json::to_string(other).unwrap_or_default();
                let compressed = compress_json_value(other);
                if compressed == *other {
                    (compressed, None, "passthrough".to_string())
                } else {
                    let key = self.store_ccr(&text, "json");
                    (
                        json!({
                            "contextx_compressed": compressed,
                            "ccr_key": key,
                            "note": "Original JSON is available through contextx_retrieve."
                        }),
                        Some(key),
                        "json".to_string(),
                    )
                }
            }
        }
    }

    fn store_ccr(&mut self, content: &str, content_type: &str) -> String {
        while self.ccr_order.len() >= DEFAULT_CCR_LIMIT {
            if let Some(oldest) = self.ccr_order.pop_front() {
                self.ccr.remove(&oldest);
            }
        }
        let key = format!("ccr_{}", short_hash(content));
        self.ccr.insert(
            key.clone(),
            CcrEntry {
                content: content.to_string(),
                content_type: content_type.to_string(),
                expires_at: Utc::now() + Duration::minutes(DEFAULT_CCR_TTL_MINUTES),
            },
        );
        self.ccr_order.retain(|existing| existing != &key);
        self.ccr_order.push_back(key.clone());
        key
    }

    fn evict_expired(&mut self) {
        let now = Utc::now();
        self.ccr.retain(|_, entry| entry.expires_at > now);
        self.ccr_order.retain(|key| self.ccr.contains_key(key));
    }

    fn push_event(&mut self, event: Event) {
        while self.events.len() >= EVENT_LIMIT {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

fn add_agent_stats(stats: &mut AgentStats, event: &Event) {
    if event.kind == "output" {
        stats.output_tokens += event.compressed_tokens;
    } else {
        stats.original_tokens += event.original_tokens;
        stats.compressed_tokens += event.compressed_tokens;
        stats.requests += 1;
    }
}

fn window_stats(
    events: &[Event],
    window_start: DateTime<Utc>,
    window_minutes: Option<usize>,
) -> WindowStats {
    let mut stats = WindowStats::default();
    for event in events {
        if event.kind == "output" {
            stats.output_tokens += event.compressed_tokens;
        } else {
            stats.original_tokens += event.original_tokens;
            stats.compressed_tokens += event.compressed_tokens;
            stats.requests += 1;
        }
    }
    stats.savings_pct = savings_pct(stats.original_tokens, stats.compressed_tokens);
    let elapsed_minutes = (Utc::now() - window_start).num_seconds().max(1) as f64 / 60.0;
    stats.burn_tokens_per_minute =
        (stats.compressed_tokens + stats.output_tokens) as f64 / elapsed_minutes;
    stats.estimated_minutes_left = window_minutes.and_then(|limit| {
        if stats.burn_tokens_per_minute > 0.0 {
            Some(limit as f64 - elapsed_minutes)
        } else {
            None
        }
    });
    stats
}

fn choose_algorithm(text: &str, requested: &str) -> String {
    if requested != "auto" {
        return requested.to_string();
    }
    let trimmed = text.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        "json".to_string()
    } else if looks_like_code(text) {
        "code".to_string()
    } else if estimate_tokens(text) > 220 || text.lines().count() > 20 {
        "prose".to_string()
    } else {
        "passthrough".to_string()
    }
}

fn looks_like_code(text: &str) -> bool {
    let needles = [
        "fn ",
        "def ",
        "class ",
        "import ",
        "use ",
        "package ",
        "const ",
        "let ",
        "function ",
        "#include",
    ];
    needles.iter().any(|needle| text.contains(needle))
}

fn compress_json_text(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => serde_json::to_string_pretty(&compress_json_value(&value))
            .unwrap_or_else(|_| text.to_string()),
        Err(_) => compress_prose_text(text),
    }
}

fn compress_json_value(value: &Value) -> Value {
    match value {
        Value::Array(items) if items.len() > 8 => json!({
            "contextx_summary": {
                "type": "array",
                "items": items.len(),
                "kept": "first 3 and last 3"
            },
            "first": &items[..3],
            "last": &items[items.len() - 3..]
        }),
        Value::Object(map) if map.len() > 16 => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            json!({
                "contextx_summary": {
                    "type": "object",
                    "keys": keys.len(),
                    "key_names": keys
                }
            })
        }
        other => other.clone(),
    }
}

fn compress_code_text(text: &str) -> String {
    let mut kept = Vec::new();
    let mut omitted = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        let important = trimmed.starts_with("use ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("#include")
            || trimmed.starts_with("pub ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("enum ")
            || trimmed.contains("ERROR")
            || trimmed.contains("TODO")
            || trimmed.contains("panic!");
        if important || kept.len() < 12 {
            kept.push(line.to_string());
        } else {
            omitted += 1;
        }
    }
    if omitted == 0 {
        text.to_string()
    } else {
        format!(
            "{}\n\n// [ContextX compressed {} less-informative lines; retrieve CCR for full source]",
            kept.join("\n"),
            omitted
        )
    }
}

fn compress_prose_text(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= 20 && estimate_tokens(text) <= 220 {
        return text.to_string();
    }
    let head = lines.iter().take(8).copied().collect::<Vec<_>>().join("\n");
    let tail = lines.iter().rev().take(4).copied().collect::<Vec<_>>();
    let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
    format!(
        "[ContextX prose summary: {} lines, ~{} tokens]\n{}\n\n...\n\n{}",
        lines.len(),
        estimate_tokens(text),
        head,
        tail
    )
}

fn summarize_algorithms(algorithms: &[String]) -> String {
    let mut unique = algorithms.to_vec();
    unique.sort();
    unique.dedup();
    unique.join("+")
}

pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let whitespace = text.split_whitespace().count();
    usize::max(1, usize::max(chars / 4, whitespace))
}

fn message_tokens(message: &Message) -> usize {
    estimate_tokens(&message.role) + estimate_tokens(&content_to_string(&message.content))
}

fn content_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn savings_pct(original: usize, compressed: usize) -> f64 {
    if original == 0 {
        0.0
    } else {
        let saved = original.saturating_sub(compressed) as f64;
        ((saved / original as f64) * 1000.0).round() / 10.0
    }
}

fn hash_json<T: Serialize>(value: &T) -> String {
    short_hash(&serde_json::to_string(value).unwrap_or_default())
}

fn short_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn auto() -> String {
    "auto".to_string()
}

fn unknown_agent() -> String {
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compresses_and_retrieves_large_prose() {
        let mut engine = ContextXEngine::default();
        let text = (0..80)
            .map(|i| format!("line {i} important context"))
            .collect::<Vec<_>>()
            .join("\n");
        let response = engine.compress(CompressRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: Value::String(text.clone()),
            }],
            model: "test".to_string(),
            provider: "test".to_string(),
            budget_tokens: None,
            algorithm: "auto".to_string(),
            agent: "test".to_string(),
            project_path: None,
        });
        assert!(response.compressed_tokens < response.original_tokens);
        assert_eq!(response.ccr_keys.len(), 1);
        let retrieved = engine.retrieve(&response.ccr_keys);
        assert_eq!(retrieved.contents[0].content, text);
    }

    #[test]
    fn compresses_large_json_array() {
        let mut engine = ContextXEngine::default();
        let content = json!(
            (0..20)
                .map(|i| json!({"id": i, "value": format!("item-{i}")}))
                .collect::<Vec<_>>()
        );
        let response = engine.compress(CompressRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content,
            }],
            model: "test".to_string(),
            provider: "test".to_string(),
            budget_tokens: None,
            algorithm: "auto".to_string(),
            agent: "test".to_string(),
            project_path: None,
        });
        assert_eq!(response.algorithm_used, "json");
        assert_eq!(response.ccr_keys.len(), 1);
    }

    #[test]
    fn auto_mode_does_not_expand_short_context() {
        let mut engine = ContextXEngine::default();
        let text = "short direct prompt; do not add overhead".to_string();
        let response = engine.compress(test_request(Value::String(text.clone())));

        assert_eq!(response.algorithm_used, "passthrough");
        assert!(response.ccr_keys.is_empty());
        assert_eq!(response.original_tokens, response.compressed_tokens);
        assert_eq!(response.compressed_messages[0].content, Value::String(text));
    }

    #[test]
    fn code_compression_keeps_power_lines_and_retrieves_full_source() {
        let mut engine = ContextXEngine::default();
        let mut lines = vec![
            "use std::collections::HashMap;".to_string(),
            "pub struct UsageTracker { total: usize }".to_string(),
            "fn compress_context(input: &str) -> String {".to_string(),
            "    // TODO: preserve important implementation notes".to_string(),
        ];
        lines.extend((0..120).map(|i| format!("    let filler_{i} = {i};")));
        lines.push("    panic!(\"critical failure marker\");".to_string());
        lines.push("}".to_string());
        let source = lines.join("\n");

        let response = engine.compress(test_request(Value::String(source.clone())));
        let compressed = response.compressed_messages[0].content.as_str().unwrap();

        assert_eq!(response.algorithm_used, "code");
        assert!(response.savings_pct > 50.0);
        assert!(compressed.contains("use std::collections::HashMap;"));
        assert!(compressed.contains("pub struct UsageTracker"));
        assert!(compressed.contains("fn compress_context"));
        assert!(compressed.contains("TODO"));
        assert!(compressed.contains("panic!"));

        let retrieved = engine.retrieve(&response.ccr_keys);
        assert_eq!(retrieved.contents.len(), 1);
        assert_eq!(retrieved.contents[0].content, source);
    }

    #[test]
    fn before_after_suite_saves_tokens_while_preserving_retrieval() {
        let prose = Value::String(
            (0..160)
                .map(|i| {
                    format!("project context line {i}: usage monitor compression intelligence")
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let json_content = json!(
            (0..80)
                .map(|i| json!({"id": i, "title": format!("task-{i}"), "status": "open", "owner": "alpha-x"}))
                .collect::<Vec<_>>()
        );
        let code = Value::String(
            [
                "import os",
                "class ContextX:",
                "def compress(self, messages):",
                "    return messages",
            ]
            .into_iter()
            .chain((0..100).map(|_| "value = 'repeated detail for local context only'"))
            .collect::<Vec<_>>()
            .join("\n"),
        );

        let cases = [prose, json_content, code];
        for case in cases {
            let mut engine = ContextXEngine::default();
            let response = engine.compress(test_request(case));

            assert!(
                response.compressed_tokens < response.original_tokens,
                "expected token reduction, got original={} compressed={} algorithm={}",
                response.original_tokens,
                response.compressed_tokens,
                response.algorithm_used
            );
            assert!(!response.ccr_keys.is_empty());
            assert!(response.savings_pct > 20.0);

            let retrieved = engine.retrieve(&response.ccr_keys);
            assert_eq!(retrieved.contents.len(), response.ccr_keys.len());
        }
    }

    fn test_request(content: Value) -> CompressRequest {
        CompressRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content,
            }],
            model: "test".to_string(),
            provider: "test".to_string(),
            budget_tokens: None,
            algorithm: "auto".to_string(),
            agent: "test".to_string(),
            project_path: None,
        }
    }
}
