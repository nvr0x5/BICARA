//! Subscription providers via the user's own CLIs.

use std::process::Stdio;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::shellenv::{find_binary, user_env};
use crate::{err, ChatRequest, Result};

fn base_command(bin: &std::path::Path) -> Command {
    let mut c = Command::new(bin);
    c.env_clear()
        .envs(user_env())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Run outside any project so no CLAUDE.md / AGENTS.md from the cwd leaks into answers.
    c.current_dir(std::env::temp_dir());
    c
}

// ------------------------------------------------------------------ Claude Code CLI

struct Warm {
    key: (String, String, String), // (binary, model, system)
    child: Child,
}

/// Keeps one `claude -p` process started and waiting on stdin, so a request only pays for
/// the model call, not for CLI startup (~3-5 s).
#[derive(Default)]
pub struct ClaudeCliPool {
    warm: Mutex<Option<Warm>>,
}

fn spawn_claude(bin: &std::path::Path, model: &str, system: &str) -> std::io::Result<Child> {
    let mut c = base_command(bin);
    c.args([
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--no-session-persistence",
        "--tools",
        "",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--model",
        model,
        "--system-prompt",
        system,
    ]);
    c.spawn()
}

impl ClaudeCliPool {
    pub async fn prewarm(&self, binary: Option<&str>, model: &str, system: &str) {
        let Some(bin) = find_binary("claude", binary) else {
            return;
        };
        let key = (
            bin.to_string_lossy().into_owned(),
            model.to_string(),
            system.to_string(),
        );
        let mut g = self.warm.lock().await;
        if let Some(w) = g.as_mut() {
            if w.key == key && matches!(w.child.try_wait(), Ok(None)) {
                return;
            }
        }
        if let Ok(child) = spawn_claude(&bin, model, system) {
            *g = Some(Warm { key, child });
        }
    }

    pub async fn complete(
        &self,
        binary: Option<&str>,
        model: &str,
        req: &ChatRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<String> {
        let bin = find_binary("claude", binary)
            .ok_or_else(|| err("CLI `claude` tidak ditemukan. Install Claude Code lalu jalankan `claude` sekali untuk login."))?;
        let key = (
            bin.to_string_lossy().into_owned(),
            model.to_string(),
            req.system.clone(),
        );

        let reused = {
            let mut g = self.warm.lock().await;
            match g.take() {
                Some(mut w) if w.key == key => match w.child.try_wait() {
                    Ok(None) => Some(w.child),
                    _ => None,
                },
                _ => None,
            }
        };
        let mut child = match reused {
            Some(c) => c,
            None => spawn_claude(&bin, model, &req.system)?,
        };

        let result = run_claude(&mut child, req, on_delta).await;

        // Get the next process ready in the background.
        if let Ok(next) = spawn_claude(&bin, model, &req.system) {
            *self.warm.lock().await = Some(Warm { key, child: next });
        }
        result
    }
}

async fn run_claude(
    child: &mut Child,
    req: &ChatRequest,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| err("stdin claude tidak tersedia"))?;
    stdin.write_all(req.user.as_bytes()).await?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| err("stdout claude tidak tersedia"))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut streamed = String::new();
    let mut final_text: Option<String> = None;
    let mut error: Option<String> = None;
    while let Some(line) = lines.next_line().await? {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("stream_event") => {
                let ev = &v["event"];
                if ev["type"] == "content_block_delta" && ev["delta"]["type"] == "text_delta" {
                    if let Some(t) = ev["delta"]["text"].as_str() {
                        streamed.push_str(t);
                        on_delta(t);
                    }
                } else if ev["type"] == "message_stop" && !streamed.is_empty() {
                    // The answer is complete; the CLI still does post-turn bookkeeping for
                    // a few seconds, which the user should not wait for.
                    let _ = child.start_kill();
                    return Ok(streamed);
                }
            }
            Some("result") => {
                if v["is_error"].as_bool() == Some(true) || v["subtype"] != "success" {
                    error = Some(
                        v["result"]
                            .as_str()
                            .unwrap_or("claude CLI gagal")
                            .to_string(),
                    );
                } else {
                    final_text = v["result"].as_str().map(str::to_string);
                }
            }
            _ => {}
        }
    }
    let status = child.wait().await?;
    if let Some(e) = error {
        return Err(err(format!("claude: {e}")));
    }
    if !status.success() && final_text.is_none() {
        let mut s = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut s).await;
        }
        return Err(err(format!("claude keluar dengan {status}: {}", s.trim())));
    }
    let text = final_text.unwrap_or(streamed.clone());
    if streamed.is_empty() && !text.is_empty() {
        on_delta(&text);
    }
    Ok(text)
}

// ------------------------------------------------------------------ Codex CLI

/// `codex exec --json` prints JSONL events; the answer arrives as an `agent_message` item.
/// Codex has no system-prompt flag, so instructions are prepended to the prompt.
pub async fn codex_complete(
    binary: Option<&str>,
    model: Option<&str>,
    req: &ChatRequest,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<String> {
    let bin = find_binary("codex", binary).ok_or_else(|| {
        err("CLI `codex` tidak ditemukan. Install Codex CLI lalu jalankan `codex login`.")
    })?;
    let mut c = base_command(&bin);
    c.args([
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
    ]);
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        c.args(["--model", m]);
    }
    c.arg("-"); // read prompt from stdin
    let mut child = c.spawn()?;
    let prompt = format!(
        "<instructions>\n{}\n</instructions>\n\n{}",
        req.system, req.user
    );
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| err("stdin codex tidak tersedia"))?;
    stdin.write_all(prompt.as_bytes()).await?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| err("stdout codex tidak tersedia"))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut text = String::new();
    let mut error: Option<String> = None;
    while let Some(line) = lines.next_line().await? {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let ty = v["type"].as_str().unwrap_or_default();
        let item = &v["item"];
        if ty == "item.completed"
            && (item["type"] == "agent_message" || item["item_type"] == "assistant_message")
        {
            if let Some(t) = item["text"].as_str() {
                if !text.is_empty() {
                    text.push('\n');
                    on_delta("\n");
                }
                text.push_str(t);
                on_delta(t);
            }
        } else if ty == "error" || ty == "turn.failed" {
            error = Some(
                v["message"]
                    .as_str()
                    .or(v["error"]["message"].as_str())
                    .unwrap_or("codex gagal")
                    .to_string(),
            );
        }
    }
    let status = child.wait().await?;
    if let Some(e) = error {
        return Err(err(format!("codex: {e}")));
    }
    if !status.success() && text.is_empty() {
        let mut s = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut s).await;
        }
        return Err(err(format!("codex keluar dengan {status}: {}", s.trim())));
    }
    Ok(text)
}

/// One-off Claude CLI check: runs a tiny prompt and reports which concrete model the alias
/// (e.g. `sonnet`) resolved to, read from the CLI's `system/init` event.
pub async fn claude_probe(binary: Option<&str>, model: &str) -> Result<(String, Option<String>)> {
    let bin = find_binary("claude", binary)
        .ok_or_else(|| err("CLI `claude` tidak ditemukan. Install Claude Code lalu jalankan `claude` sekali untuk login."))?;
    let mut child = spawn_claude(&bin, model, "Reply with exactly: OK")?;
    let mut stdin = child.stdin.take().ok_or_else(|| err("stdin claude tidak tersedia"))?;
    stdin.write_all(b"ping").await?;
    drop(stdin);
    let stdout = child.stdout.take().ok_or_else(|| err("stdout claude tidak tersedia"))?;
    let mut lines = BufReader::new(stdout).lines();
    let (mut resolved, mut text) = (None, String::new());
    while let Some(line) = lines.next_line().await? {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        match v["type"].as_str() {
            Some("system") if v["subtype"] == "init" => resolved = v["model"].as_str().map(str::to_string),
            Some("assistant") => {
                if resolved.is_none() {
                    resolved = v["message"]["model"].as_str().map(str::to_string);
                }
            }
            Some("stream_event") => {
                let ev = &v["event"];
                if ev["type"] == "content_block_delta" {
                    if let Some(t) = ev["delta"]["text"].as_str() {
                        text.push_str(t);
                    }
                } else if ev["type"] == "message_stop" && !text.is_empty() {
                    let _ = child.start_kill();
                    return Ok((text, resolved));
                }
            }
            Some("result") => {
                if v["is_error"].as_bool() == Some(true) || v["subtype"] != "success" {
                    return Err(err(format!("claude: {}", v["result"].as_str().unwrap_or("gagal"))));
                }
                return Ok((v["result"].as_str().unwrap_or(&text).to_string(), resolved));
            }
            _ => {}
        }
    }
    let mut s = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut s).await;
    }
    Err(err(format!("claude berhenti tanpa jawaban: {}", s.trim())))
}
