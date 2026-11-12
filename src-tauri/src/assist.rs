//! Live transcript + AI actions (Bantu jawab, Tanya, Ringkas, Pertanyaan lanjutan).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bicara_llm::ChatRequest;
use bicara_stt::Utterance;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;
use crate::settings::Settings;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Assist,
    Ask,
    Summary,
    Followup,
    /// Post-meeting minutes.
    Notulen,
    /// Interview preparation (STAR answers to common HRD questions).
    Prep,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Assist => "assist",
            Action::Ask => "ask",
            Action::Summary => "summary",
            Action::Followup => "followup",
            Action::Notulen => "notulen",
            Action::Prep => "prep",
        }
    }
}

#[derive(Clone, Serialize)]
struct AiStart {
    id: u64,
    action: Action,
    title: String,
    provider: String,
}

#[derive(Clone, Serialize)]
struct AiDelta {
    id: u64,
    delta: String,
}

#[derive(Clone, Serialize)]
struct AiDone {
    id: u64,
    text: String,
    error: Option<String>,
    ms: u64,
}

// ---------------------------------------------------------------- question detection

const QUESTION_STARTS: &[&str] = &[
    "apa",
    "apakah",
    "bagaimana",
    "gimana",
    "kenapa",
    "mengapa",
    "kapan",
    "di mana",
    "dimana",
    "siapa",
    "berapa",
    "bisakah",
    "bisa",
    "boleh",
    "mohon",
    "tolong",
    "coba",
    "ceritakan",
    "jelaskan",
    "menurut",
    "seberapa",
    "sejauh mana",
    "what",
    "how",
    "why",
    "when",
    "where",
    "who",
    "can you",
    "could you",
    "tell me",
    "do you",
    "have you",
];
const QUESTION_ANYWHERE: &[&str] = &[
    "bisa jelaskan",
    "bisa dijelaskan",
    "bisa ceritakan",
    "tolong jelaskan",
    "coba ceritakan",
    "menurut anda",
    "menurut kamu",
    "menurutmu",
    "gimana menurut",
    "bagaimana menurut",
    "apa pendapat",
    "boleh tahu",
    "kira-kira berapa",
];

/// Heuristic: does this utterance from "Mereka" ask something the user should answer?
pub fn looks_like_question(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    if t.split_whitespace().count() < 3 {
        return false;
    }
    if t.contains('?') {
        return true;
    }
    if QUESTION_ANYWHERE.iter().any(|p| t.contains(p)) {
        return true;
    }
    // A sentence that starts with a question word.
    t.split(['.', '!', ',']).map(str::trim).any(|s| {
        QUESTION_STARTS
            .iter()
            .any(|q| s.starts_with(q) && s[q.len()..].starts_with(' '))
            && s.split_whitespace().count() >= 3
    })
}

// ---------------------------------------------------------------- transcript

fn source_label(s: &str) -> &'static str {
    if s == "mic" { "Saya" } else { "Mereka" }
}

fn fmt_time(t: f64) -> String {
    let s = t as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

pub fn transcript_text(lines: &[Utterance], max_chars: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut n = 0;
    for u in lines.iter().rev() {
        let l = format!(
            "[{}] {}: {}",
            fmt_time(u.start),
            source_label(&u.source),
            u.text
        );
        n += l.len() + 1;
        if n > max_chars {
            out.push("[… transkrip sebelumnya dipotong …]".into());
            break;
        }
        out.push(l);
    }
    out.reverse();
    out.join("\n")
}

/// Called by the STT worker for each finished utterance.
pub fn on_utterance(app: &AppHandle, u: Utterance) {
    let state = app.state::<AppState>();
    if u.partial {
        if state.settings.lock().unwrap().live_partials {
            let _ = app.emit("transcript-partial", &u);
        }
        return;
    }
    state.transcript.lock().unwrap().push(u.clone());
    if let Some(mid) = *state.current_meeting.lock().unwrap() {
        let _ = state
            .store
            .lock()
            .unwrap()
            .add_utterance(mid, &u.source, &u.text, u.start, u.end);
    }
    let _ = app.emit("transcript", &u);
    let target = state.settings.lock().unwrap().translate_to.clone();
    if !target.is_empty() {
        translate(app.clone(), &u, target);
    }

    if u.source == "app" && looks_like_question(&u.text) {
        let _ = app.emit("question-detected", &u.text);
        *state.last_question.lock().unwrap() = Some(u.text.clone());
        let auto = state.settings.lock().unwrap().auto_assist;
        if auto {
            let mut last = state.last_auto_assist.lock().unwrap();
            // Avoid stacking answers when several question fragments arrive in a row.
            if last.is_none_or(|t| t.elapsed().as_secs_f32() > 4.0) {
                *last = Some(Instant::now());
                drop(last);
                run(app.clone(), Action::Assist, Some(u.text.clone()));
            }
        }
    }
}

#[tauri::command]
pub fn get_transcript(state: State<'_, AppState>) -> Vec<Utterance> {
    state.transcript.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_transcript(state: State<'_, AppState>) {
    state.transcript.lock().unwrap().clear();
    *state.last_question.lock().unwrap() = None;
}

// ---------------------------------------------------------------- prompts

pub fn system_prompt(s: &Settings) -> String {
    let p = s.preset();
    let register = if s.language == "en" {
        if s.register == "santai" {
            "Write all answers in natural, casual-professional English."
        } else {
            "Write all answers in polite, formal business English."
        }
    } else if s.register == "santai" {
        "Gunakan bahasa Indonesia santai sehari-hari (aku/kamu), tetap sopan dan profesional."
    } else {
        "Gunakan bahasa Indonesia baku dan sopan (Bapak/Ibu, Anda)."
    };
    let mut out = format!(
        "{}\n\n\
         Kamu membantu PENGGUNA secara diam-diam selama percakapan live. Di transkrip, \"Saya\" adalah \
         pengguna dan \"Mereka\" adalah lawan bicara. Transkrip berasal dari speech-to-text otomatis \
         dan bisa salah dengar: tafsirkan maksudnya, jangan mengomentari kesalahan transkrip.\n\
         {register} Ikuti bahasa yang dipakai lawan bicara (Indonesia atau Inggris) kecuali diminta lain.\n\
         Tulis singkat dan langsung bisa dipakai. Jangan membuka dengan basa-basi. Format markdown \
         sederhana (paragraf pendek dan bullet) saja.",
        p.system_prompt.trim()
    );
    if !p.context.trim().is_empty() {
        out.push_str(&format!(
            "\n\nKonteks dari pengguna (CV, agenda, catatan, dll.):\n<konteks>\n{}\n</konteks>",
            p.context.trim()
        ));
    }
    out
}

fn user_prompt(action: Action, transcript: &str, input: Option<&str>) -> (String, String) {
    let tr = if transcript.is_empty() {
        "(belum ada transkrip)".to_string()
    } else {
        transcript.to_string()
    };
    match action {
        Action::Assist => {
            let q = input.unwrap_or("(pertanyaan/topik terakhir dari Mereka di transkrip)");
            (
                "Bantu jawab".into(),
                format!(
                    "<transkrip>\n{tr}\n</transkrip>\n\nYang perlu Saya tanggapi sekarang: «{q}»\n\n\
                     Tulis apa yang sebaiknya Saya ucapkan: jawaban langsung yang natural untuk \
                     diucapkan (2–4 kalimat), lalu maksimal 3 poin pendukung singkat yang bisa \
                     ditambahkan jika perlu."
                ),
            )
        }
        Action::Ask => (
            "Tanya".into(),
            format!(
                "<transkrip>\n{tr}\n</transkrip>\n\nPertanyaan Saya untuk kamu (bukan untuk diucapkan): {}",
                input.unwrap_or("")
            ),
        ),
        Action::Summary => (
            "Ringkasan".into(),
            format!(
                "<transkrip>\n{tr}\n</transkrip>\n\nBuat notulen singkat dari percakapan sejauh ini dengan \
                 bagian: **Ringkasan** (3–5 poin), **Keputusan**, **Action items** (tugas — PIC — \
                 tenggat, tulis \"?\" jika tidak disebut), **Pertanyaan terbuka**. Lewati bagian yang kosong."
            ),
        ),
        Action::Notulen | Action::Prep => unreachable!("built by meeting module"),
        Action::Followup => (
            "Pertanyaan lanjutan".into(),
            format!(
                "<transkrip>\n{tr}\n</transkrip>\n\nSarankan 3 pertanyaan lanjutan yang tajam dan relevan \
                 yang bisa Saya ajukan sekarang, masing-masing satu kalimat, diurutkan dari yang paling berguna."
            ),
        ),
    }
}

fn provider_label(cfg: &bicara_llm::ProviderConfig) -> String {
    use bicara_llm::ProviderConfig::*;
    match cfg {
        ClaudeCli { model, .. } => format!("Claude (langganan) · {model}"),
        CodexCli { model, .. } => format!(
            "Codex (langganan){}",
            model
                .as_ref()
                .map(|m| format!(" · {m}"))
                .unwrap_or_default()
        ),
        Anthropic { model, .. } => format!("Anthropic API · {model}"),
        OpenAiCompatible { label, model, .. } => format!("{label} · {model}"),
    }
}

// ---------------------------------------------------------------- run

/// Called with the final text when an AI request succeeds.
pub type OnDone = Box<dyn FnOnce(&AppHandle, &str) + Send>;

/// Stream one LLM request to the UI (`ai-start` / `ai-delta` / `ai-done`) and, when a meeting
/// is given, save the answer in its history.
pub fn stream_llm(
    app: AppHandle,
    meeting_id: Option<i64>,
    action: Action,
    title: String,
    input: Option<String>,
    req: ChatRequest,
    on_done: Option<OnDone>,
) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let state = app.state::<AppState>();
    let cfg = state.settings.lock().unwrap().provider.clone();
    let key = cfg.key_account().and_then(|a| crate::settings::api_key(&a));
    let llm = state.llm.clone();
    let _ = app.emit(
        "ai-start",
        AiStart {
            id,
            action,
            title: title.clone(),
            provider: provider_label(&cfg),
        },
    );
    tauri::async_runtime::spawn(async move {
        let t0 = Instant::now();
        let app2 = app.clone();
        let mut on_delta = move |d: &str| {
            let _ = app2.emit(
                "ai-delta",
                AiDelta {
                    id,
                    delta: d.to_string(),
                },
            );
        };
        let res = llm
            .complete(&cfg, key.as_deref(), &req, &mut on_delta)
            .await;
        let ms = t0.elapsed().as_millis() as u64;
        let done = match res {
            Ok(text) => {
                if let Some(mid) = meeting_id {
                    let st = app.state::<AppState>();
                    let _ = st.store.lock().unwrap().add_answer(
                        mid,
                        action.as_str(),
                        &title,
                        input.as_deref(),
                        &text,
                    );
                }
                if let Some(f) = on_done {
                    f(&app, &text);
                }
                AiDone {
                    id,
                    text,
                    error: None,
                    ms,
                }
            }
            Err(e) => AiDone {
                id,
                text: String::new(),
                error: Some(e.to_string()),
                ms,
            },
        };
        let _ = app.emit("ai-done", done);
    });
    id
}

fn emit_error(app: &AppHandle, action: Action, title: String, msg: &str) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let _ = app.emit(
        "ai-start",
        AiStart {
            id,
            action,
            title,
            provider: String::new(),
        },
    );
    let _ = app.emit(
        "ai-done",
        AiDone {
            id,
            text: String::new(),
            error: Some(msg.into()),
            ms: 0,
        },
    );
    id
}

/// Meeting documents relevant to `query`, wrapped for the prompt (empty if none).
pub fn documents_block(
    app: &AppHandle,
    meeting_id: Option<i64>,
    query: &str,
    budget: usize,
) -> String {
    let Some(mid) = meeting_id else {
        return String::new();
    };
    let st = app.state::<AppState>();
    let ctx = st
        .store
        .lock()
        .unwrap()
        .document_context(mid, query, budget)
        .unwrap_or_default();
    if ctx.is_empty() {
        String::new()
    } else {
        format!(
            "Dokumen konteks meeting ini (gunakan bila relevan, sebutkan nama dokumennya):\n{ctx}\n\n"
        )
    }
}

/// Live actions during a meeting (or a quick session without one).
pub fn run(app: AppHandle, action: Action, input: Option<String>) -> u64 {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let meeting_id = *state.current_meeting.lock().unwrap();
    let input = match (action, input) {
        (Action::Assist, None) => state.last_question.lock().unwrap().clone(),
        (_, i) => i,
    };
    let transcript = transcript_text(&state.transcript.lock().unwrap(), 12_000);
    if transcript.is_empty() && input.is_none() && action != Action::Ask {
        let title = user_prompt(action, "", None).0;
        return emit_error(
            &app,
            action,
            title,
            "Belum ada percakapan. Mulai meeting dan tunggu lawan bicara berbicara.",
        );
    }
    let (title, user) = user_prompt(action, &transcript, input.as_deref());
    // Retrieval query: the question plus the latest part of the conversation.
    let tail: String = transcript
        .chars()
        .rev()
        .take(600)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let query = format!("{} {}", input.as_deref().unwrap_or(""), tail);
    let docs = documents_block(&app, meeting_id, &query, 24_000);
    let req = ChatRequest {
        // System prompt stays identical across requests so the warm CLI process can be reused.
        system: system_prompt(&settings),
        user: format!("{docs}{user}"),
        max_tokens: if action == Action::Summary { 1500 } else { 700 },
    };
    stream_llm(app.clone(), meeting_id, action, title, input, req, None)
}

/// Keep a subscription CLI process warm for the current preset (saves seconds per answer).
pub fn prewarm(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().unwrap().clone();
    let llm = state.llm.clone();
    tauri::async_runtime::spawn(async move {
        llm.prewarm(&s.provider, &system_prompt(&s)).await;
    });
}

#[tauri::command]
pub fn ai_action(app: AppHandle, action: Action, input: Option<String>) -> u64 {
    run(app, action, input.filter(|s| !s.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_indonesian_questions() {
        for q in [
            "Bisa jelaskan pengalaman Anda memimpin tim",
            "Apa kelebihan dan kekurangan Anda?",
            "Ceritakan tentang diri Anda.",
            "Oke. Kenapa Anda tertarik dengan posisi ini",
            "Jadi gimana menurut kamu soal timeline-nya",
            "Berapa ekspektasi gaji Anda",
            "Can you walk me through your last project",
        ] {
            assert!(looks_like_question(q), "{q}");
        }
    }

    #[test]
    fn ignores_statements() {
        for s in [
            "Baik, terima kasih.",
            "Jadi kita lanjut ke agenda berikutnya.",
            "Saya setuju dengan usulan tadi.",
            "Apa",
            "Bisa",
        ] {
            assert!(!looks_like_question(s), "{s}");
        }
    }
}

// ---------------------------------------------------------------- live translation

#[derive(Clone, Serialize)]
struct Translation {
    source: String,
    start: f64,
    text: String,
}

/// One translation at a time, in transcript order (keeps CLI processes and rate limits sane).
static TRANSLATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Translates one finished transcript line and emits `translation`.
/// Uses the fastest model of the configured provider family (Haiku for Claude subscription).
fn translate(app: AppHandle, u: &Utterance, target: String) {
    let state = app.state::<AppState>();
    let mut cfg = state.settings.lock().unwrap().provider.clone();
    if let bicara_llm::ProviderConfig::ClaudeCli { model, .. } = &mut cfg {
        *model = "haiku".into();
    }
    let key = cfg.key_account().and_then(|a| crate::settings::api_key(&a));
    let llm = state.llm.clone();
    let (source, start, text) = (u.source.clone(), u.start, u.text.clone());
    let lang = if target == "en" { "English" } else { "Bahasa Indonesia" };
    let req = ChatRequest {
        system: format!(
            "You are a live meeting interpreter. Translate the user's line into natural, concise {lang}. \
             Keep names, product names and technical terms as they are. If the line is already in {lang}, \
             return it unchanged. Output only the translation, no quotes or notes."
        ),
        user: text,
        max_tokens: 400,
    };
    tauri::async_runtime::spawn(async move {
        let _turn = TRANSLATE_LOCK.lock().await;
        let mut sink = |_: &str| {};
        match llm.complete(&cfg, key.as_deref(), &req, &mut sink).await {
            Ok(t) if !t.trim().is_empty() => {
                let _ = app.emit(
                    "translation",
                    Translation {
                        source,
                        start,
                        text: t.trim().to_string(),
                    },
                );
            }
            Ok(_) => {}
            Err(e) => eprintln!("[bicara] translation failed: {e}"),
        }
    });
}
