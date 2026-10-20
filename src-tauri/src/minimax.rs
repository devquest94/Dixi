//! MiniMax text-completion provider: request building and response parsing.
//!
//! MiniMax deviates from the OpenAI and Anthropic conventions in two ways
//! that silently corrupt a dictation when they are handled naively:
//!
//! 1. **The `base_resp` envelope.** MiniMax reports failures *inside* an
//!    HTTP 200 body as `base_resp: { status_code: number, status_msg: string }`.
//!    `status_code == 0` means success. A client that only inspects the HTTP
//!    status treats a rate limit, an exhausted quota or a billing problem as
//!    a success and pastes the error envelope into the user's text.
//! 2. **Reasoning is not the answer.** MiniMax delivers the model's reasoning
//!    ("thinking") separately from the answer, both in the response body
//!    (`reasoning_content`, `reasoning_details`, `thinking`) and in the SSE
//!    stream (`thinking_delta`). Reasoning must never reach the user's text.
//!
//! Reasoning depth is requested with `reasoning_effort` (OpenAI format) or
//! `output_config.effort` (Anthropic format). The value is never `none`:
//! `MiniMax-M3.1-Flash` always reasons and answers HTTP 400 when reasoning
//! is asked to be disabled. `MiniMax-M3` has reasoning off by default, which
//! makes it the better fit for short dictation clean-ups.
//!
//! This module is pure: it builds request bodies and parses responses. It
//! performs no network I/O.

use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error as StdError;
use std::fmt;

/// `base_resp.status_code` reported when the request was rate limited.
const STATUS_RATE_LIMITED: i64 = 1002;
/// `base_resp.status_code` reported when the plan's quota is exhausted.
const STATUS_QUOTA_EXHAUSTED: i64 = 2056;
/// `base_resp.status_code` reported when the account has no usable balance.
const STATUS_BILLING_REQUIRED: i64 = 1008;

/// Reasoning depth requested from MiniMax.
///
/// MiniMax accepts `low | medium | high | xhigh | max`. `max` is the API
/// default, but it is slow and expensive, so Dixi defaults to
/// [`MiniMaxEffort::Low`].
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MiniMaxEffort {
    /// Fastest reasoning. The sensible default for dictation clean-ups.
    #[default]
    Low,
    /// Moderate reasoning depth.
    Medium,
    /// Deep reasoning.
    High,
    /// Deeper than `high`.
    XHigh,
    /// The API default. Slow and expensive.
    Max,
}

impl MiniMaxEffort {
    /// The wire representation of the effort level.
    ///
    /// Never returns `none`: `MiniMax-M3.1-Flash` always reasons and answers
    /// HTTP 400 when reasoning is disabled.
    pub fn as_str(&self) -> &'static str {
        match self {
            MiniMaxEffort::Low => "low",
            MiniMaxEffort::Medium => "medium",
            MiniMaxEffort::High => "high",
            MiniMaxEffort::XHigh => "xhigh",
            MiniMaxEffort::Max => "max",
        }
    }
}

impl fmt::Display for MiniMaxEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The request format spoken by a MiniMax endpoint.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MiniMaxApiFormat {
    /// OpenAI-compatible Chat Completions (`/v1/chat/completions`).
    #[default]
    OpenAi,
    /// Anthropic-compatible Messages (`/anthropic/v1/messages`).
    Anthropic,
}

/// Connection and request settings for a MiniMax text completion.
///
/// The module never reads the API key itself; it is carried here so the
/// caller can build the `Authorization` header from the same settings.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MiniMaxConfig {
    /// Base URL including the version segment, e.g. `https://api.minimax.io/v1`
    /// (OpenAI format) or `https://api.minimax.io/anthropic` (Anthropic
    /// format).
    pub base_url: String,
    /// Model id, e.g. `MiniMax-M3` (reasoning off by default) or
    /// `MiniMax-M3.1-Flash` (always reasons).
    pub model: String,
    /// API key sent as a bearer token.
    pub api_key: String,
    /// Reasoning depth requested from the model.
    pub effort: MiniMaxEffort,
    /// Output token budget.
    pub max_tokens: u32,
    /// Per-request timeout, in seconds.
    pub timeout_secs: u64,
}

impl Default for MiniMaxConfig {
    /// The recommended starting point for dictation clean-ups: `MiniMax-M3`
    /// (reasoning off by default) with the cheapest reasoning depth.
    fn default() -> Self {
        Self {
            base_url: "https://api.minimax.io/v1".to_string(),
            model: "MiniMax-M3".to_string(),
            api_key: String::new(),
            effort: MiniMaxEffort::Low,
            max_tokens: 4096,
            timeout_secs: 60,
        }
    }
}

/// Failure modes of a MiniMax text completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiniMaxError {
    /// `base_resp.status_code == 1002`: too many requests.
    RateLimited,
    /// `base_resp.status_code == 2056`: the plan's quota is exhausted.
    QuotaExhausted,
    /// `base_resp.status_code == 1008`: the account has no usable balance.
    BillingRequired,
    /// Any other non-zero `base_resp.status_code`, or a non-success HTTP
    /// response that carried no recognizable envelope.
    Api {
        /// The status code reported by the server.
        status_code: i64,
        /// The message reported by the server.
        status_msg: String,
    },
    /// The response body could not be understood.
    Malformed(String),
    /// The server rejected the requested reasoning settings (HTTP 400).
    ReasoningNotSupported,
    /// The request never completed (connection failure, timeout, ...).
    Transport(String),
}

impl MiniMaxError {
    /// Whether the request may be retried unchanged.
    ///
    /// Only a rate limit is worth retrying: a quota, a billing problem or a
    /// rejected request would fail again the same way.
    pub fn is_retryable(&self) -> bool {
        matches!(self, MiniMaxError::RateLimited)
    }
}

impl fmt::Display for MiniMaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MiniMaxError::RateLimited => formatter.write_str("MiniMax rate limit reached"),
            MiniMaxError::QuotaExhausted => formatter.write_str("MiniMax quota exhausted"),
            MiniMaxError::BillingRequired => {
                formatter.write_str("MiniMax account has no usable balance")
            }
            MiniMaxError::Api {
                status_code,
                status_msg,
            } => write!(formatter, "MiniMax API error {status_code}: {status_msg}"),
            MiniMaxError::Malformed(details) => {
                write!(formatter, "Malformed MiniMax response: {details}")
            }
            MiniMaxError::ReasoningNotSupported => {
                formatter.write_str("MiniMax rejected the requested reasoning settings")
            }
            MiniMaxError::Transport(details) => {
                write!(formatter, "MiniMax request failed: {details}")
            }
        }
    }
}

impl StdError for MiniMaxError {}

/// A single decoded event of a MiniMax SSE stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiniMaxChunk {
    /// A fragment of the answer. Safe to paste into the user's text.
    Text(String),
    /// A fragment of the model's reasoning. Must never be merged into the
    /// answer.
    Thinking(String),
    /// The stream ended.
    Done,
}

/// Inspect the `base_resp` envelope and turn a non-zero status into an error.
///
/// MiniMax reports failures inside an HTTP 200 body, so a client that only
/// looks at the HTTP status treats them as successes. A body without a
/// `base_resp` envelope carries no status and is accepted.
fn check_base_resp(body: &Value) -> Result<(), MiniMaxError> {
    let Some(envelope) = body.get("base_resp") else {
        return Ok(());
    };
    if envelope.is_null() {
        return Ok(());
    }
    let Some(raw_status) = envelope.get("status_code") else {
        return Ok(());
    };

    let status_code = match raw_status {
        Value::Number(number) => number.as_i64().ok_or_else(|| {
            MiniMaxError::Malformed(format!("base_resp.status_code is not an integer: {number}"))
        })?,
        Value::String(text) => text.trim().parse::<i64>().map_err(|_| {
            MiniMaxError::Malformed(format!("base_resp.status_code is not an integer: {text}"))
        })?,
        other => {
            return Err(MiniMaxError::Malformed(format!(
                "base_resp.status_code has an unexpected type: {other}"
            )))
        }
    };

    if status_code == 0 {
        return Ok(());
    }

    let status_msg = envelope
        .get("status_msg")
        .and_then(|message| message.as_str())
        .unwrap_or_default()
        .to_string();

    let error = match status_code {
        STATUS_RATE_LIMITED => MiniMaxError::RateLimited,
        STATUS_QUOTA_EXHAUSTED => MiniMaxError::QuotaExhausted,
        STATUS_BILLING_REQUIRED => MiniMaxError::BillingRequired,
        _ => MiniMaxError::Api {
            status_code,
            status_msg,
        },
    };

    // The message may quote response content, so only the code is logged.
    warn!("MiniMax request failed with base_resp.status_code {status_code}");
    Err(error)
}

/// Read the assistant text of a message object, ignoring every reasoning
/// field (`reasoning_content`, `reasoning_details`, `thinking`).
fn message_text(message: &Value) -> Option<String> {
    match message.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                match part.get("type").and_then(|kind| kind.as_str()) {
                    Some("thinking") | Some("reasoning") => continue,
                    _ => {}
                }
                if let Some(chunk) = part.get("text").and_then(|text| text.as_str()) {
                    text.push_str(chunk);
                }
            }
            Some(text)
        }
        _ => None,
    }
}

/// Extract the assistant text from a MiniMax OpenAI-compatible
/// chat-completion body.
///
/// A body whose `base_resp.status_code` is not `0` is rejected even though
/// the HTTP status was 200, because MiniMax reports its failures inside the
/// payload. Reasoning content is discarded; only the answer is returned.
pub fn parse_chat_completion(body: &Value) -> Result<String, MiniMaxError> {
    check_base_resp(body)?;

    let choice = body
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first());

    let text = match choice.and_then(|choice| choice.get("message")) {
        Some(message) => message_text(message),
        None => None,
    }
    .or_else(|| {
        choice
            .and_then(|choice| choice.get("text"))
            .and_then(|text| text.as_str())
            .map(str::to_string)
    })
    .or_else(|| {
        body.get("reply")
            .and_then(|reply| reply.as_str())
            .map(str::to_string)
    });

    match text {
        Some(text) => Ok(text),
        None => Err(MiniMaxError::Malformed(
            "response carries no assistant text".to_string(),
        )),
    }
}

/// Extract the assistant text from an Anthropic-compatible `/v1/messages`
/// body, concatenating only `content` blocks whose `type == "text"` and
/// skipping `thinking` blocks.
pub fn parse_anthropic_messages(body: &Value) -> Result<String, MiniMaxError> {
    check_base_resp(body)?;

    let blocks = body
        .get("content")
        .and_then(|content| content.as_array())
        .ok_or_else(|| MiniMaxError::Malformed("response carries no content blocks".to_string()))?;

    let mut text = String::new();
    for block in blocks {
        if block.get("type").and_then(|kind| kind.as_str()) != Some("text") {
            continue;
        }
        if let Some(chunk) = block.get("text").and_then(|text| text.as_str()) {
            text.push_str(chunk);
        }
    }
    Ok(text)
}

/// Read the reasoning fragment of an SSE payload.
///
/// MiniMax names it `thinking` in the Anthropic-compatible stream and
/// `reasoning_content` in the OpenAI-compatible one; some servers reuse
/// `text` for it.
fn thinking_text(value: &Value) -> Option<String> {
    value
        .get("thinking")
        .and_then(|thinking| thinking.as_str())
        .or_else(|| {
            value
                .get("reasoning_content")
                .and_then(|reasoning| reasoning.as_str())
        })
        .or_else(|| value.get("text").and_then(|text| text.as_str()))
        .map(str::to_string)
}

/// Map an SSE delta object to a chunk.
///
/// `delta.type` is `text_delta` for the answer and `thinking_delta` for the
/// model's reasoning; the two must never be merged.
fn chunk_from_delta(delta: &Value) -> Option<MiniMaxChunk> {
    match delta.get("type").and_then(|kind| kind.as_str()) {
        Some("text_delta") => delta
            .get("text")
            .and_then(|text| text.as_str())
            .map(|text| MiniMaxChunk::Text(text.to_string())),
        Some("thinking_delta") | Some("reasoning_delta") => {
            thinking_text(delta).map(MiniMaxChunk::Thinking)
        }
        // A block start carries the same field names as the delta.
        Some("text") => delta
            .get("text")
            .and_then(|text| text.as_str())
            .map(|text| MiniMaxChunk::Text(text.to_string())),
        Some("thinking") | Some("reasoning") => thinking_text(delta).map(MiniMaxChunk::Thinking),
        // Untyped delta: infer the kind from the field that is present.
        None => {
            if let Some(text) = delta.get("text").and_then(|text| text.as_str()) {
                Some(MiniMaxChunk::Text(text.to_string()))
            } else {
                thinking_text(delta).map(MiniMaxChunk::Thinking)
            }
        }
        Some(_) => None,
    }
}

/// Map an OpenAI-compatible streaming payload (`choices[0].delta`) to a
/// chunk.
fn chunk_from_openai_delta(value: &Value) -> Option<MiniMaxChunk> {
    let delta = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))?;

    if let Some(text) = delta.get("content").and_then(|content| content.as_str()) {
        if !text.is_empty() {
            return Some(MiniMaxChunk::Text(text.to_string()));
        }
    }
    if let Some(reasoning) = delta
        .get("reasoning_content")
        .and_then(|reasoning| reasoning.as_str())
    {
        return Some(MiniMaxChunk::Thinking(reasoning.to_string()));
    }
    if let Some(details) = delta
        .get("reasoning_details")
        .and_then(|details| details.as_array())
    {
        let mut reasoning = String::new();
        for detail in details {
            if let Some(chunk) = detail.get("text").and_then(|text| text.as_str()) {
                reasoning.push_str(chunk);
            }
        }
        if !reasoning.is_empty() {
            return Some(MiniMaxChunk::Thinking(reasoning));
        }
    }
    delta
        .get("content")
        .and_then(|content| content.as_str())
        .map(|content| MiniMaxChunk::Text(content.to_string()))
}

/// Map a decoded SSE payload to a chunk.
///
/// Both the Anthropic-compatible shape (`delta.type` of `text_delta` or
/// `thinking_delta`) and the OpenAI-compatible shape (`choices[].delta`) are
/// understood, so the same parser serves both endpoint families.
fn chunk_from_event(value: &Value) -> Option<MiniMaxChunk> {
    if let Some(delta) = value.get("delta") {
        if let Some(chunk) = chunk_from_delta(delta) {
            return Some(chunk);
        }
    }

    let by_type = match value.get("type").and_then(|kind| kind.as_str()) {
        Some("text_delta") => value
            .get("text")
            .and_then(|text| text.as_str())
            .map(|text| MiniMaxChunk::Text(text.to_string())),
        Some("thinking_delta") => thinking_text(value).map(MiniMaxChunk::Thinking),
        Some("content_block_start") => value.get("content_block").and_then(chunk_from_delta),
        _ => None,
    };

    by_type.or_else(|| chunk_from_openai_delta(value))
}

/// Parse one SSE `data: {...}` line into a typed chunk.
///
/// A `text_delta` becomes [`MiniMaxChunk::Text`], a `thinking_delta` becomes
/// [`MiniMaxChunk::Thinking`] and must be kept apart from the answer, and
/// `data: [DONE]` becomes [`MiniMaxChunk::Done`]. Keep-alives, comments and
/// unrelated events are ignored (`Ok(None)`).
pub fn parse_sse_data_line(line: &str) -> Result<Option<MiniMaxChunk>, MiniMaxError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let Some(payload) = trimmed.strip_prefix("data:") else {
        // A comment, a keep-alive or a non-data field (`event:`, `id:`).
        return Ok(None);
    };
    let payload = payload.trim();
    if payload.is_empty() {
        return Ok(None);
    }
    if payload == "[DONE]" {
        return Ok(Some(MiniMaxChunk::Done));
    }

    let value: Value = serde_json::from_str(payload)
        .map_err(|error| MiniMaxError::Malformed(format!("invalid SSE payload: {error}")))?;

    Ok(chunk_from_event(&value))
}

/// Build the request body for the given API format.
///
/// The OpenAI format carries the reasoning depth in `reasoning_effort` and the
/// Anthropic format in `output_config.effort`. The value is never `none`.
pub fn build_request_body(
    format: MiniMaxApiFormat,
    config: &MiniMaxConfig,
    system_prompt: &str,
    user_text: &str,
) -> Value {
    match format {
        MiniMaxApiFormat::OpenAi => json!({
            "model": config.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_text},
            ],
            // Never "none": MiniMax-M3.1-Flash always reasons and answers
            // HTTP 400 when reasoning is disabled.
            "reasoning_effort": config.effort.as_str(),
            "max_tokens": config.max_tokens,
        }),
        MiniMaxApiFormat::Anthropic => json!({
            "model": config.model,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_text},
            ],
            "max_tokens": config.max_tokens,
            "output_config": {"effort": config.effort.as_str()},
        }),
    }
}

/// The HTTP path to POST to, relative to [`MiniMaxConfig::base_url`].
///
/// The OpenAI base URL already carries the version segment
/// (`https://api.minimax.io/v1`), the Anthropic base URL does not
/// (`https://api.minimax.io/anthropic`).
pub fn endpoint_path(format: MiniMaxApiFormat) -> &'static str {
    match format {
        MiniMaxApiFormat::OpenAi => "/chat/completions",
        MiniMaxApiFormat::Anthropic => "/v1/messages",
    }
}

/// The full URL of the endpoint for a configuration.
///
/// A duplicated version segment is collapsed, so both
/// `https://api.minimax.io/anthropic` and `https://api.minimax.io/anthropic/v1`
/// resolve to the same Messages endpoint.
pub fn request_url(config: &MiniMaxConfig, format: MiniMaxApiFormat) -> String {
    let base = config.base_url.trim_end_matches('/');
    let path = endpoint_path(format);
    if path.starts_with("/v1") && base.ends_with("/v1") {
        return format!("{}{}", base, &path[3..]);
    }
    format!("{base}{path}")
}

/// Best-effort human readable message of an error body.
fn error_message(body: &Value) -> String {
    body.get("base_resp")
        .and_then(|envelope| envelope.get("status_msg"))
        .and_then(|message| message.as_str())
        .or_else(|| {
            body.get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
        })
        .or_else(|| {
            body.get("error")
                .and_then(|error| error.as_str())
        })
        .or_else(|| body.get("message").and_then(|message| message.as_str()))
        .unwrap_or_default()
        .to_string()
}

/// Map a non-success HTTP response to an error.
///
/// A payload that carries a `base_resp` envelope is classified by that
/// envelope first, because MiniMax reports its errors there. A 400 that
/// complains about the reasoning settings means the model refuses the
/// requested effort, which is a configuration problem rather than a
/// malformed request.
pub fn classify_http_error(status: u16, body: &Value) -> MiniMaxError {
    if let Err(error) = check_base_resp(body) {
        return error;
    }
    if status == 400 {
        let message = error_message(body);
        let lowered = message.to_lowercase();
        if lowered.contains("thinking")
            || lowered.contains("reasoning")
            || lowered.contains("effort")
        {
            return MiniMaxError::ReasoningNotSupported;
        }
    }
    if status == 429 {
        return MiniMaxError::RateLimited;
    }
    MiniMaxError::Api {
        status_code: status as i64,
        status_msg: error_message(body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_body(raw: &str) -> Value {
        serde_json::from_str(raw).expect("fixture must be valid JSON")
    }

    fn config() -> MiniMaxConfig {
        MiniMaxConfig {
            base_url: "https://api.minimax.io/v1".to_string(),
            model: "MiniMax-M3".to_string(),
            api_key: "test-key".to_string(),
            effort: MiniMaxEffort::Low,
            max_tokens: 4096,
            timeout_secs: 60,
        }
    }

    #[test]
    fn base_resp_failure_is_reported_even_with_http_200() {
        let body = json_body(
            r#"{"choices":[{"message":{"content":"partial answer"}}],
                "base_resp":{"status_code":1002,"status_msg":"rate limit"}}"#,
        );
        assert_eq!(parse_chat_completion(&body), Err(MiniMaxError::RateLimited));
    }

    #[test]
    fn base_resp_known_status_codes_map_to_their_error() {
        let cases = [
            (1002, MiniMaxError::RateLimited),
            (2056, MiniMaxError::QuotaExhausted),
            (1008, MiniMaxError::BillingRequired),
        ];
        for (status_code, expected) in cases {
            let body = json_body(&format!(
                r#"{{"choices":[],"base_resp":{{"status_code":{status_code},"status_msg":"nope"}}}}"#
            ));
            assert_eq!(parse_chat_completion(&body), Err(expected));
        }
    }

    #[test]
    fn base_resp_unknown_status_code_becomes_an_api_error() {
        let body = json_body(
            r#"{"choices":[],"base_resp":{"status_code":1234,"status_msg":"boom"}}"#,
        );
        assert_eq!(
            parse_chat_completion(&body),
            Err(MiniMaxError::Api {
                status_code: 1234,
                status_msg: "boom".to_string()
            })
        );
    }

    #[test]
    fn base_resp_success_returns_the_answer() {
        let body = json_body(
            r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"Hola mundo"},
                "finish_reason":"stop"}],"base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(parse_chat_completion(&body).unwrap(), "Hola mundo");
    }

    #[test]
    fn reasoning_content_is_never_part_of_the_answer() {
        let body = json_body(
            r#"{"choices":[{"message":{"role":"assistant","content":"Hola mundo",
                "reasoning_content":"SECRET CHAIN OF THOUGHT",
                "reasoning_details":[{"type":"text","text":"SECRET DETAIL"}],
                "thinking":"SECRET THINKING"}}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        let text = parse_chat_completion(&body).unwrap();
        assert_eq!(text, "Hola mundo");
        assert!(!text.contains("SECRET"));
    }

    #[test]
    fn reasoning_blocks_are_skipped_in_array_content() {
        let body = json_body(
            r#"{"choices":[{"message":{"content":[
                {"type":"thinking","text":"SECRET THINKING"},
                {"type":"text","text":"Hola"},
                {"type":"text","text":" mundo"}]}}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        let text = parse_chat_completion(&body).unwrap();
        assert_eq!(text, "Hola mundo");
        assert!(!text.contains("SECRET"));
    }

    #[test]
    fn sse_text_delta_is_answer_text() {
        let chunk = parse_sse_data_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hola"}}"#,
        )
        .unwrap();
        assert_eq!(chunk, Some(MiniMaxChunk::Text("Hola".to_string())));
    }

    #[test]
    fn sse_thinking_delta_is_never_answer_text() {
        let chunk = parse_sse_data_line(
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"razonamiento"}}"#,
        )
        .unwrap();
        assert_eq!(
            chunk,
            Some(MiniMaxChunk::Thinking("razonamiento".to_string()))
        );
        assert_ne!(
            chunk,
            Some(MiniMaxChunk::Text("razonamiento".to_string()))
        );
    }

    #[test]
    fn sse_done_marker_ends_the_stream() {
        assert_eq!(
            parse_sse_data_line("data: [DONE]").unwrap(),
            Some(MiniMaxChunk::Done)
        );
    }

    #[test]
    fn sse_ignores_keep_alives_and_unrelated_events() {
        assert_eq!(parse_sse_data_line("").unwrap(), None);
        assert_eq!(parse_sse_data_line(": keep-alive").unwrap(), None);
        assert_eq!(parse_sse_data_line("event: ping").unwrap(), None);
        assert_eq!(parse_sse_data_line("data: ").unwrap(), None);
        assert_eq!(
            parse_sse_data_line(r#"data: {"type":"message_start","message":{}}"#).unwrap(),
            None
        );
    }

    #[test]
    fn sse_openai_style_delta_keeps_reasoning_apart() {
        let answer = parse_sse_data_line(
            r#"data: {"choices":[{"delta":{"content":"Hola"}}]}"#,
        )
        .unwrap();
        assert_eq!(answer, Some(MiniMaxChunk::Text("Hola".to_string())));

        let reasoning =
            parse_sse_data_line(r#"data: {"choices":[{"delta":{"reasoning_content":"razonamiento"}}]}"#)
                .unwrap();
        assert_eq!(
            reasoning,
            Some(MiniMaxChunk::Thinking("razonamiento".to_string()))
        );
    }

    #[test]
    fn sse_invalid_payload_is_reported_as_malformed() {
        let error = parse_sse_data_line("data: {not json").unwrap_err();
        assert!(matches!(error, MiniMaxError::Malformed(_)));
    }

    #[test]
    fn anthropic_messages_keeps_text_blocks_and_drops_thinking() {
        let body = json_body(
            r#"{"type":"message","role":"assistant","content":[
                {"type":"thinking","thinking":"razonamiento"},
                {"type":"text","text":"Hola"},
                {"type":"text","text":" mundo"}]}"#,
        );
        let text = parse_anthropic_messages(&body).unwrap();
        assert_eq!(text, "Hola mundo");
        assert!(!text.contains("razonamiento"));
    }

    #[test]
    fn anthropic_messages_reports_a_missing_content_array() {
        let body = json_body(r#"{"type":"message","role":"assistant"}"#);
        assert!(matches!(
            parse_anthropic_messages(&body),
            Err(MiniMaxError::Malformed(_))
        ));
    }

    #[test]
    fn anthropic_messages_rejects_a_failing_base_resp() {
        let body = json_body(
            r#"{"content":[{"type":"text","text":"Hola"}],"base_resp":{"status_code":1002,"status_msg":"rate limit"}}"#,
        );
        assert_eq!(
            parse_anthropic_messages(&body),
            Err(MiniMaxError::RateLimited)
        );
    }

    #[test]
    fn openai_request_carries_reasoning_effort_and_never_none() {
        let body = build_request_body(MiniMaxApiFormat::OpenAi, &config(), "system", "user");
        assert_eq!(body["model"], "MiniMax-M3");
        assert_eq!(body["reasoning_effort"], "low");
        assert_ne!(body["reasoning_effort"], "none");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "user");
    }

    #[test]
    fn anthropic_request_carries_output_config_effort() {
        let body = build_request_body(MiniMaxApiFormat::Anthropic, &config(), "system", "user");
        assert_eq!(body["model"], "MiniMax-M3");
        assert_eq!(body["system"], "system");
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "user");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn effort_levels_map_to_the_documented_strings() {
        assert_eq!(MiniMaxEffort::Low.as_str(), "low");
        assert_eq!(MiniMaxEffort::Medium.as_str(), "medium");
        assert_eq!(MiniMaxEffort::High.as_str(), "high");
        assert_eq!(MiniMaxEffort::XHigh.as_str(), "xhigh");
        assert_eq!(MiniMaxEffort::Max.as_str(), "max");
        for effort in [
            MiniMaxEffort::Low,
            MiniMaxEffort::Medium,
            MiniMaxEffort::High,
            MiniMaxEffort::XHigh,
            MiniMaxEffort::Max,
        ] {
            assert_ne!(effort.as_str(), "none");
        }
    }

    #[test]
    fn endpoint_paths_match_the_documented_base_urls() {
        assert_eq!(endpoint_path(MiniMaxApiFormat::OpenAi), "/chat/completions");
        assert_eq!(endpoint_path(MiniMaxApiFormat::Anthropic), "/v1/messages");
    }

    #[test]
    fn request_url_normalizes_the_version_segment() {
        let mut settings = config();
        assert_eq!(
            request_url(&settings, MiniMaxApiFormat::OpenAi),
            "https://api.minimax.io/v1/chat/completions"
        );

        settings.base_url = "https://api.minimax.io/anthropic".to_string();
        assert_eq!(
            request_url(&settings, MiniMaxApiFormat::Anthropic),
            "https://api.minimax.io/anthropic/v1/messages"
        );

        settings.base_url = "https://api.minimax.io/anthropic/v1".to_string();
        assert_eq!(
            request_url(&settings, MiniMaxApiFormat::Anthropic),
            "https://api.minimax.io/anthropic/v1/messages"
        );
    }

    #[test]
    fn only_rate_limit_is_retryable() {
        assert!(MiniMaxError::RateLimited.is_retryable());
        assert!(!MiniMaxError::QuotaExhausted.is_retryable());
        assert!(!MiniMaxError::BillingRequired.is_retryable());
        assert!(!MiniMaxError::Api {
            status_code: 1234,
            status_msg: "boom".to_string()
        }
        .is_retryable());
        assert!(!MiniMaxError::Malformed("bad".to_string()).is_retryable());
        assert!(!MiniMaxError::ReasoningNotSupported.is_retryable());
        assert!(!MiniMaxError::Transport("timeout".to_string()).is_retryable());
    }

    #[test]
    fn http_400_about_reasoning_is_reported_as_unsupported() {
        let body = json_body(r#"{"error":{"message":"thinking is not supported for this model"}}"#);
        assert_eq!(
            classify_http_error(400, &body),
            MiniMaxError::ReasoningNotSupported
        );
    }

    #[test]
    fn http_429_is_a_retryable_rate_limit() {
        let body = json_body(r#"{"error":{"message":"slow down"}}"#);
        let error = classify_http_error(429, &body);
        assert_eq!(error, MiniMaxError::RateLimited);
        assert!(error.is_retryable());
    }

    #[test]
    fn http_error_prefers_the_base_resp_envelope() {
        let body = json_body(
            r#"{"base_resp":{"status_code":2056,"status_msg":"quota"},"error":{"message":"ignored"}}"#,
        );
        assert_eq!(
            classify_http_error(400, &body),
            MiniMaxError::QuotaExhausted
        );
    }
}
