// SPDX-License-Identifier: GPL-3.0-or-later
//! One chat completion against the sidecar (R5 §4.5 `local` provider, W-02).
//!
//! Request and reply handling ported from TypeWhisper's `LlamaCppChatClient`
//! (GPL-3.0-or-later, docs/fork/PORTS.md): instructions in the system turn
//! and the text in the user turn, no thinking (`enable_thinking: false`), a
//! reply cut off at the token limit is an error (never paste half a
//! sentence), context overflow has its own error. Added: temperature 0 with a
//! fixed seed, the JSON schema, llama-server's `timings`, and cancellation:
//! dropping the request closes the connection, which makes `llama-server`
//! stop generating and free the slot (RT-12, measured in RS-06).
//!
//! The reply is **streamed** (SSE) even though we only use the whole text:
//! b11624 checks a non-streaming client for a hang-up only once per second
//! (`HTTP_POLLING_SECONDS = 1` in `server-context.cpp`), so Esc left the slot
//! busy for up to a second (RS-06 measured +590 ms p50). With streaming the
//! server notices the closed socket at the next token.

use std::time::{Duration, Instant};

pub use hf_hub::api::tokio::CancellationToken;
use serde::Deserialize;
use serde_json::{json, Value};

use super::process::MODEL_ALIAS;
use super::transport::Transport;

/// Fixed sampling seed (RS-07).
pub const SEED: u64 = 42;

/// What to send.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    /// `response_format` (JSON schema) or `None` for plain text.
    pub response_format: Option<Value>,
    /// Worked examples sent as earlier (user, assistant) turns between the
    /// system prompt and `user` (transforms, OT2-D). Usually empty.
    pub examples: Vec<(String, String)>,
}

impl ChatRequest {
    /// The JSON body (OpenAI chat completions + llama.cpp extensions).
    pub fn body(&self) -> Value {
        let mut messages = vec![json!({ "role": "system", "content": self.system })];
        for (user, assistant) in &self.examples {
            messages.push(json!({ "role": "user", "content": user }));
            messages.push(json!({ "role": "assistant", "content": assistant }));
        }
        messages.push(json!({ "role": "user", "content": self.user }));
        let mut body = json!({
            "model": MODEL_ALIAS,
            "messages": messages,
            "temperature": 0,
            "seed": SEED,
            "max_tokens": self.max_tokens,
            "stream": true,
            "chat_template_kwargs": { "enable_thinking": false },
        });
        if let Some(rf) = &self.response_format {
            body["response_format"] = rf.clone();
        }
        body
    }
}

/// A finished reply with llama-server's timings.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatReply {
    pub content: String,
    /// Wall time of the HTTP request.
    pub total: Duration,
    pub prompt_tokens: u32,
    pub prompt_ms: f64,
    pub generated_tokens: u32,
    pub generation_ms: f64,
    /// Tokens served from the prompt cache.
    pub cached_tokens: u32,
}

/// Why a completion failed. Every variant makes the caller fail open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    /// The caller cancelled (Esc): the connection was closed.
    Cancelled,
    /// Connection failed or dropped (server gone, crashed mid-take).
    Transport(String),
    /// Request timed out.
    Timeout,
    /// The input does not fit the context.
    InputTooLong,
    /// The reply hit `max_tokens`.
    Truncated,
    /// HTTP error from the server.
    Server(u16, String),
    /// No content in the reply.
    Empty,
    /// The body was not the expected JSON.
    Malformed,
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatError::Cancelled => write!(f, "cancelled"),
            ChatError::Transport(m) => write!(f, "transport: {m}"),
            ChatError::Timeout => write!(f, "timed out"),
            ChatError::InputTooLong => write!(f, "input exceeds the context"),
            ChatError::Truncated => write!(f, "reply cut off at the token limit"),
            ChatError::Server(code, m) => write!(f, "server {code}: {m}"),
            ChatError::Empty => write!(f, "empty reply"),
            ChatError::Malformed => write!(f, "malformed reply"),
        }
    }
}

#[derive(Deserialize)]
struct Reply {
    choices: Vec<Choice>,
    #[serde(default)]
    timings: Option<Timings>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct Timings {
    #[serde(default)]
    prompt_n: i64,
    #[serde(default)]
    prompt_ms: f64,
    #[serde(default)]
    predicted_n: i64,
    #[serde(default)]
    predicted_ms: f64,
    #[serde(default)]
    cache_n: i64,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn clamp_u32(n: i64) -> u32 {
    u32::try_from(n.max(0)).unwrap_or(u32::MAX)
}

/// Parses a reply body (TypeWhisper `LlamaCppChatClient.parse`).
pub fn parse(status: u16, body: &[u8], total: Duration) -> Result<ChatReply, ChatError> {
    if status != 200 {
        let detail = serde_json::from_slice::<ErrorBody>(body)
            .ok()
            .map(|b| b.error);
        let message = detail
            .as_ref()
            .and_then(|d| d.message.clone())
            .unwrap_or_else(|| String::from_utf8_lossy(body).chars().take(200).collect());
        let too_long = detail.as_ref().and_then(|d| d.kind.as_deref())
            == Some("exceed_context_size_error")
            || message
                .to_ascii_lowercase()
                .contains("exceeds the available context size");
        return Err(if too_long {
            ChatError::InputTooLong
        } else {
            ChatError::Server(status, message)
        });
    }
    let reply: Reply = serde_json::from_slice(body).map_err(|_| ChatError::Malformed)?;
    let choice = reply.choices.into_iter().next().ok_or(ChatError::Empty)?;
    if choice.finish_reason.as_deref() == Some("length") {
        return Err(ChatError::Truncated);
    }
    let content = choice.message.content.ok_or(ChatError::Empty)?;
    let t = reply.timings.unwrap_or_default();
    Ok(ChatReply {
        content,
        total,
        prompt_tokens: clamp_u32(t.prompt_n),
        prompt_ms: t.prompt_ms,
        generated_tokens: clamp_u32(t.predicted_n),
        generation_ms: t.predicted_ms,
        cached_tokens: clamp_u32(t.cache_n),
    })
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    timings: Option<Timings>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    #[serde(default)]
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

/// Accumulates a streamed (SSE) reply.
#[derive(Default)]
pub struct StreamState {
    buf: Vec<u8>,
    content: String,
    finish_reason: Option<String>,
    timings: Option<Timings>,
    done: bool,
    saw_chunk: bool,
}

impl StreamState {
    /// Feeds raw bytes; complete `data:` events are parsed.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), ChatError> {
        self.buf.extend_from_slice(bytes);
        while let Some(i) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=i).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                self.done = true;
                continue;
            }
            let chunk: Chunk = serde_json::from_str(data).map_err(|_| ChatError::Malformed)?;
            self.saw_chunk = true;
            if let Some(c) = chunk.choices.into_iter().next() {
                if let Some(t) = c.delta.and_then(|d| d.content) {
                    self.content.push_str(&t);
                }
                if c.finish_reason.is_some() {
                    self.finish_reason = c.finish_reason;
                }
            }
            if chunk.timings.is_some() {
                self.timings = chunk.timings;
            }
        }
        Ok(())
    }

    /// The finished reply.
    pub fn finish(self, total: Duration) -> Result<ChatReply, ChatError> {
        if !self.saw_chunk {
            return Err(ChatError::Empty);
        }
        if self.finish_reason.as_deref() == Some("length") {
            return Err(ChatError::Truncated);
        }
        if self.finish_reason.is_none() && !self.done {
            // The stream ended without a finish: the server went away.
            return Err(ChatError::Transport("stream ended early".into()));
        }
        let t = self.timings.unwrap_or_default();
        Ok(ChatReply {
            content: self.content,
            total,
            prompt_tokens: clamp_u32(t.prompt_n),
            prompt_ms: t.prompt_ms,
            generated_tokens: clamp_u32(t.predicted_n),
            generation_ms: t.predicted_ms,
            cached_tokens: clamp_u32(t.cache_n),
        })
    }
}

/// Sends one completion. Returns [`ChatError::Cancelled`] as soon as
/// `cancel` fires; the request future is dropped, which closes the
/// connection.
pub async fn complete(
    client: &reqwest::Client,
    request: &ChatRequest,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<ChatReply, ChatError> {
    let t0 = Instant::now();
    let send = async {
        let mut resp = client
            .post(Transport::url("/v1/chat/completions"))
            .json(&request.body())
            .timeout(timeout)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = resp.status().as_u16();
        if status != 200 {
            let body = resp.bytes().await.map_err(map_reqwest)?;
            return parse(status, &body, t0.elapsed());
        }
        let mut state = StreamState::default();
        while let Some(chunk) = resp.chunk().await.map_err(map_reqwest)? {
            state.feed(&chunk)?;
        }
        state.finish(t0.elapsed())
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(ChatError::Cancelled),
        r = send => r,
    }
}

fn map_reqwest(e: reqwest::Error) -> ChatError {
    if e.is_timeout() {
        ChatError::Timeout
    } else {
        // `Display` of a reqwest error carries no payload.
        ChatError::Transport(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_has_system_and_user_turns_without_thinking() {
        let r = ChatRequest {
            system: "S".into(),
            user: "U".into(),
            max_tokens: 70,
            response_format: Some(super::super::prompt::response_format()),
            examples: Vec::new(),
        };
        let b = r.body();
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][0]["content"], "S");
        assert_eq!(b["messages"][1]["role"], "user");
        assert_eq!(b["messages"][1]["content"], "U");
        assert_eq!(b["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(b["temperature"], 0);
        assert_eq!(b["max_tokens"], 70);
        assert_eq!(b["stream"], true);
        assert_eq!(b["model"], "local");
        assert_eq!(b["response_format"]["type"], "json_schema");
        let plain = ChatRequest {
            response_format: None,
            examples: Vec::new(),
            ..r
        };
        assert!(plain.body().get("response_format").is_none());
    }

    #[test]
    fn parses_answers_timings_and_server_errors() {
        let ok = br#"{"choices":[{"finish_reason":"stop","message":{"content":"Done."}}],
            "timings":{"prompt_n":12,"prompt_ms":20.5,"predicted_n":3,"predicted_ms":30.0,"cache_n":400}}"#;
        let r = parse(200, ok, Duration::from_millis(60)).unwrap();
        assert_eq!(r.content, "Done.");
        assert_eq!(
            (r.prompt_tokens, r.generated_tokens, r.cached_tokens),
            (12, 3, 400)
        );
        assert_eq!(
            parse(
                200,
                br#"{"choices":[{"finish_reason":"length","message":{"content":"Half a sen"}}]}"#,
                Duration::ZERO
            ),
            Err(ChatError::Truncated)
        );
        assert_eq!(
            parse(200, br#"{"choices":[]}"#, Duration::ZERO),
            Err(ChatError::Empty)
        );
        assert_eq!(
            parse(200, br#"{"choices":[{"message":{}}]}"#, Duration::ZERO),
            Err(ChatError::Empty)
        );
        assert_eq!(
            parse(200, b"not json", Duration::ZERO),
            Err(ChatError::Malformed)
        );
        assert_eq!(
            parse(
                400,
                br#"{"error":{"code":400,"message":"the request exceeds the available context size","type":"exceed_context_size_error"}}"#,
                Duration::ZERO
            ),
            Err(ChatError::InputTooLong)
        );
        assert_eq!(
            parse(
                401,
                br#"{"error":{"code":401,"message":"Invalid API Key","type":"authentication_error"}}"#,
                Duration::ZERO
            ),
            Err(ChatError::Server(401, "Invalid API Key".into()))
        );
    }

    #[test]
    fn streamed_replies_are_assembled_with_timings() {
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":null}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"text\\\":\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" \\\"Hi.\\\"}\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"timings\":{\"prompt_n\":5,\"predicted_n\":4,\"cache_n\":500}}\n\n",
            "data: [DONE]\n\n"
        );
        // Split anywhere: events may straddle network chunks.
        for cut in [1, 7, 40, 100, sse.len() - 3] {
            let mut st = StreamState::default();
            st.feed(&sse.as_bytes()[..cut]).unwrap();
            st.feed(&sse.as_bytes()[cut..]).unwrap();
            let r = st.finish(Duration::from_millis(9)).unwrap();
            assert_eq!(r.content, r#"{"text": "Hi."}"#);
            assert_eq!(
                (r.prompt_tokens, r.generated_tokens, r.cached_tokens),
                (5, 4, 500)
            );
        }
        let mut st = StreamState::default();
        st.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Half\"},\"finish_reason\":\"length\"}]}\n\n").unwrap();
        assert_eq!(st.finish(Duration::ZERO), Err(ChatError::Truncated));
        let mut st = StreamState::default();
        st.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hal\"}}]}\n\n")
            .unwrap();
        assert!(matches!(
            st.finish(Duration::ZERO),
            Err(ChatError::Transport(_))
        ));
        assert_eq!(
            StreamState::default().finish(Duration::ZERO),
            Err(ChatError::Empty)
        );
        let mut st = StreamState::default();
        assert_eq!(st.feed(b"data: nope\n"), Err(ChatError::Malformed));
    }

    #[tokio::test]
    async fn cancel_returns_at_once_and_closes_the_connection() {
        use tokio::io::AsyncReadExt;
        // A server that accepts and never answers; it reports when the
        // client side closes.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            loop {
                match s.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            let _ = closed_tx.send(Instant::now());
        });
        let client = Transport::new(super::super::transport::Endpoint::Tcp(port), "k".into())
            .client(Duration::from_secs(30))
            .unwrap();
        let cancel = CancellationToken::new();
        let req = ChatRequest {
            system: "s".into(),
            user: "u".into(),
            max_tokens: 8,
            response_format: None,
            examples: Vec::new(),
        };
        let c2 = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            c2.cancel();
        });
        let t0 = Instant::now();
        let r = complete(&client, &req, Duration::from_secs(30), &cancel).await;
        assert_eq!(r, Err(ChatError::Cancelled));
        let returned = t0.elapsed();
        assert!(returned < Duration::from_millis(300), "{returned:?}");
        // The client (and its pool) stays alive, as in the app.
        let closed_at = tokio::time::timeout(Duration::from_secs(2), closed_rx)
            .await
            .expect("server saw the connection close")
            .unwrap();
        assert!(closed_at.duration_since(t0) < Duration::from_millis(500));
        drop(client);
    }
}
