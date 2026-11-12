//! LLM providers for BICARA.
//!
//! | Provider          | Auth                                     | Streaming |
//! |-------------------|------------------------------------------|-----------|
//! | `ClaudeCli`       | user's own `claude` CLI login (Pro/Max)  | token deltas |
//! | `CodexCli`        | user's own `codex` CLI login (ChatGPT)   | per message |
//! | `Anthropic`       | API key                                  | token deltas |
//! | `OpenAiCompatible`| API key or none (OpenAI, Groq, OpenRouter, Ollama, LM Studio) | token deltas |
//!
//! BICARA never reads or stores subscription tokens: the CLIs are spawned as the user's own
//! programs and authenticate themselves.

mod anthropic;
mod cli;
mod openai;
pub mod shellenv;

use serde::{Deserialize, Serialize};

pub use cli::ClaudeCliPool;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("{0}")]
    Msg(String),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, LlmError>;

pub(crate) fn err(m: impl Into<String>) -> LlmError {
    LlmError::Msg(m.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderConfig {
    ClaudeCli {
        /// "haiku", "sonnet", "opus" or a full model id.
        model: String,
        #[serde(default)]
        binary: Option<String>,
    },
    CodexCli {
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        binary: Option<String>,
    },
    Anthropic {
        model: String,
        /// low | medium | high
        #[serde(default = "default_effort")]
        effort: String,
        #[serde(default)]
        base_url: Option<String>,
    },
    OpenAiCompatible {
        label: String,
        base_url: String,
        model: String,
    },
}

fn default_effort() -> String {
    "low".into()
}

impl ProviderConfig {
    /// Keychain account name for providers that need an API key.
    pub fn key_account(&self) -> Option<String> {
        match self {
            Self::Anthropic { .. } => Some("anthropic".into()),
            Self::OpenAiCompatible { label, .. } => {
                Some(format!("openai-compatible:{}", label.to_lowercase()))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

/// Shared runtime state for providers (warm CLI processes, HTTP client).
pub struct Llm {
    http: reqwest::Client,
    claude: ClaudeCliPool,
}

impl Default for Llm {
    fn default() -> Self {
        Self::new()
    }
}

impl Llm {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
            claude: ClaudeCliPool::default(),
        }
    }

    /// Start a `claude` process ahead of time so the next request skips CLI startup.
    pub async fn prewarm(&self, cfg: &ProviderConfig, system: &str) {
        if let ProviderConfig::ClaudeCli { model, binary } = cfg {
            self.claude.prewarm(binary.as_deref(), model, system).await;
        }
    }

    /// Run one request, calling `on_delta` with each text chunk. Returns the full text.
    pub async fn complete(
        &self,
        cfg: &ProviderConfig,
        api_key: Option<&str>,
        req: &ChatRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<String> {
        match cfg {
            ProviderConfig::ClaudeCli { model, binary } => {
                self.claude
                    .complete(binary.as_deref(), model, req, on_delta)
                    .await
            }
            ProviderConfig::CodexCli { model, binary } => {
                cli::codex_complete(binary.as_deref(), model.as_deref(), req, on_delta).await
            }
            ProviderConfig::Anthropic {
                model,
                effort,
                base_url,
            } => {
                let key = api_key.ok_or_else(|| err("API key Anthropic belum diisi"))?;
                anthropic::complete(
                    &self.http,
                    base_url.as_deref(),
                    key,
                    model,
                    effort,
                    req,
                    on_delta,
                )
                .await
            }
            ProviderConfig::OpenAiCompatible {
                base_url, model, ..
            } => openai::complete(&self.http, base_url, api_key, model, req, on_delta).await,
        }
    }
}

/// Status of the subscription CLIs on this machine.
#[derive(Debug, Clone, Serialize)]
pub struct CliStatus {
    pub claude: Option<String>,
    pub codex: Option<String>,
}

pub async fn detect_clis() -> CliStatus {
    CliStatus {
        claude: shellenv::find_binary("claude", None).map(|p| p.to_string_lossy().into_owned()),
        codex: shellenv::find_binary("codex", None).map(|p| p.to_string_lossy().into_owned()),
    }
}

/// Parses one SSE stream into `data:` payloads, handling events split across chunks.
pub(crate) struct SseParser {
    buf: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.push_str(&String::from_utf8_lossy(chunk));
        let mut out = Vec::new();
        while let Some(i) = self.buf.find('\n') {
            let line = self.buf[..i].trim_end_matches('\r').to_string();
            self.buf.drain(..=i);
            if let Some(d) = line.strip_prefix("data:") {
                out.push(d.trim_start().to_string());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_split_chunks() {
        let mut p = SseParser::new();
        assert!(p.feed(b"event: x\ndata: {\"a\"").is_empty());
        assert_eq!(
            p.feed(b":1}\n\ndata: [DONE]\n"),
            vec!["{\"a\":1}", "[DONE]"]
        );
    }

    #[test]
    fn config_roundtrip() {
        let c = ProviderConfig::ClaudeCli {
            model: "sonnet".into(),
            binary: None,
        };
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(j, r#"{"kind":"claude_cli","model":"sonnet","binary":null}"#);
        assert_eq!(serde_json::from_str::<ProviderConfig>(&j).unwrap(), c);
    }
}
