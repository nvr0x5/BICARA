//! Whisper model management: list, download (with progress events), load.

use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;

use crate::AppState;

pub const DEFAULT_MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

const BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// (file, label, approx size in MB)
const MODELS: &[(&str, &str, u32)] = &[
    (
        "ggml-large-v3-turbo-q5_0.bin",
        "Large v3 Turbo (q5) — direkomendasikan",
        574,
    ),
    (
        "ggml-large-v3-turbo.bin",
        "Large v3 Turbo (penuh) — paling akurat",
        1624,
    ),
    ("ggml-small.bin", "Small — untuk laptop lambat", 488),
];

#[derive(Serialize)]
pub struct ModelInfo {
    file: String,
    label: String,
    size_mb: u32,
    installed: bool,
}

#[derive(Clone, Serialize)]
struct Progress {
    file: String,
    downloaded: u64,
    total: u64,
    done: bool,
    error: Option<String>,
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models"))
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    let dir = models_dir(&app)?;
    Ok(MODELS
        .iter()
        .map(|(f, l, s)| ModelInfo {
            file: f.to_string(),
            label: l.to_string(),
            size_mb: *s,
            installed: dir.join(f).is_file(),
        })
        .collect())
}

#[tauri::command]
pub async fn download_model(app: AppHandle, file: String) -> Result<(), String> {
    if !MODELS.iter().any(|(f, _, _)| *f == file) {
        return Err("model tidak dikenal".into());
    }
    let dir = models_dir(&app)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let part = dir.join(format!("{file}.part"));
    let res: Result<(), String> = async {
        let resp = reqwest::get(format!("{BASE}/{file}"))
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("download gagal: {}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        let mut out = tokio::fs::File::create(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut got = 0u64;
        let mut last_emit = 0u64;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            out.write_all(&chunk).await.map_err(|e| e.to_string())?;
            got += chunk.len() as u64;
            if got - last_emit > 4 * 1024 * 1024 {
                last_emit = got;
                let _ = app.emit(
                    "model-download",
                    Progress {
                        file: file.clone(),
                        downloaded: got,
                        total,
                        done: false,
                        error: None,
                    },
                );
            }
        }
        out.flush().await.map_err(|e| e.to_string())?;
        tokio::fs::rename(&part, dir.join(&file))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    .await;
    let _ = app.emit(
        "model-download",
        Progress {
            file: file.clone(),
            downloaded: 0,
            total: 0,
            done: true,
            error: res.clone().err(),
        },
    );
    if res.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    res
}

/// Returns the shared transcriber, loading the configured model on first use (takes seconds).
pub fn transcriber(app: &AppHandle) -> Result<Arc<bicara_stt::LiveTranscriber>, String> {
    let state = app.state::<AppState>();
    let (model, vocab, lang) = {
        let s = state.settings.lock().unwrap();
        (
            s.stt_model.clone(),
            s.vocabulary.clone(),
            s.language.clone(),
        )
    };
    // The cache key includes the language so switching it reloads the transcriber.
    let vocab = format!("{lang}|{vocab}");
    let mut slot = state.stt.lock().unwrap();
    if let Some((loaded, vocab_loaded, t)) = slot.as_ref() {
        if *loaded == model && *vocab_loaded == vocab {
            return Ok(t.clone());
        }
    }
    let path = models_dir(app)?.join(&model);
    if !path.is_file() {
        return Err(format!(
            "Model transkripsi belum diunduh ({model}). Buka Pengaturan → Transkripsi."
        ));
    }
    let mut opts = bicara_stt::SttOptions::default();
    let terms = vocab
        .split_once('|')
        .map(|(_, v)| v)
        .unwrap_or("")
        .trim()
        .to_string();
    match lang.as_str() {
        "en" => {
            opts.language = Some("en".into());
            opts.initial_prompt = "A business meeting conversation in English.".into();
        }
        "auto" => {
            opts.language = None;
            opts.initial_prompt = "Percakapan rapat dalam bahasa Indonesia atau English.".into();
        }
        _ => {}
    }
    if !terms.is_empty() {
        opts.initial_prompt = format!(
            "{} {}: {terms}.",
            opts.initial_prompt,
            if lang == "en" { "Terms" } else { "Istilah" }
        );
    }
    log::info!("loading whisper model {model} (lang={lang})");
    let handle = app.clone();
    let t = bicara_stt::LiveTranscriber::new(&path.to_string_lossy(), opts, move |u| {
        crate::assist::on_utterance(&handle, u);
    })
    .map_err(|e| e.to_string())?;
    log::info!("whisper model loaded");
    let t = Arc::new(t);
    *slot = Some((model, vocab, t.clone()));
    Ok(t)
}

/// Load the model in the background at startup so the first "Mulai" is instant.
pub fn preload(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = transcriber(&app);
        let _ = app.emit("stt-ready", ());
    });
}

#[tauri::command]
pub fn stt_loaded(state: State<'_, AppState>) -> bool {
    state.stt.lock().unwrap().is_some()
}
