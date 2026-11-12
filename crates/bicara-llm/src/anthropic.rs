//! Anthropic Messages API over raw HTTP + SSE (there is no official Rust SDK).

use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::{err, ChatRequest, Result, SseParser};

pub async fn complete(
    http: &reqwest::Client,
    base_url: Option<&str>,
    api_key: &str,
    model: &str,
    effort: &str,
    req: &ChatRequest,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<String> {
    let base = base_url
        .unwrap_or("https://api.anthropic.com")
        .trim_end_matches('/');
    let body = json!({
        "model": model,
        "max_tokens": req.max_tokens,
        "stream": true,
        "system": req.system,
        "messages": [{ "role": "user", "content": req.user }],
        // Live assistance is latency-sensitive: keep reasoning effort low.
        "output_config": { "effort": effort },
        // Route safety-classifier refusals to a fallback model instead of failing.
        "fallbacks": "default",
    });
    let resp = http
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let t = resp.text().await.unwrap_or_default();
        return Err(err(format!("Anthropic API {status}: {t}")));
    }

    let mut parser = SseParser::new();
    let mut text = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        for data in parser.feed(&chunk?) {
            let Ok(v) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            match v["type"].as_str() {
                Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
                    if let Some(t) = v["delta"]["text"].as_str() {
                        text.push_str(t);
                        on_delta(t);
                    }
                }
                Some("message_delta") if v["delta"]["stop_reason"] == "refusal" => {
                    return Err(err("Model menolak permintaan ini (refusal)."));
                }
                Some("error") => {
                    return Err(err(format!(
                        "Anthropic: {}",
                        v["error"]["message"].as_str().unwrap_or("error")
                    )));
                }
                _ => {}
            }
        }
    }
    Ok(text)
}
