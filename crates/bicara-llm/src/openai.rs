//! OpenAI-compatible `/chat/completions` with SSE streaming.
//! Covers OpenAI, Groq, OpenRouter, Ollama (`http://localhost:11434/v1`), LM Studio.

use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::{err, ChatRequest, Result, SseParser};

pub async fn complete(
    http: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    req: &ChatRequest,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<String> {
    let body = json!({
        "model": model,
        "stream": true,
        "max_tokens": req.max_tokens,
        "messages": [
            { "role": "system", "content": req.system },
            { "role": "user", "content": req.user },
        ],
    });
    let mut r = http
        .post(format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .json(&body);
    if let Some(k) = api_key.filter(|k| !k.is_empty()) {
        r = r.bearer_auth(k);
    }
    let resp = r.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let t = resp.text().await.unwrap_or_default();
        return Err(err(format!("{base_url} {status}: {t}")));
    }
    let mut parser = SseParser::new();
    let mut text = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        for data in parser.feed(&chunk?) {
            if data == "[DONE]" {
                return Ok(text);
            }
            let Ok(v) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            if let Some(m) = v["error"]["message"].as_str() {
                return Err(err(m.to_string()));
            }
            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                text.push_str(t);
                on_delta(t);
            }
        }
    }
    Ok(text)
}
