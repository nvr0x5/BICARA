//! Persistent settings + presets (JSON in the app data dir). API keys live in the OS keychain.

use std::path::PathBuf;

use bicara_llm::ProviderConfig;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Role / instructions for the AI in this kind of conversation.
    pub system_prompt: String,
    /// Extra context pasted by the user (CV, job description, agenda, product notes…).
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: ProviderConfig,
    /// Whisper model file name inside `<app data>/models`.
    pub stt_model: String,
    /// Custom vocabulary (names, products, jargon) to help speech recognition.
    pub vocabulary: String,
    /// "formal" (Bahasa baku, Bapak/Ibu) or "santai".
    pub register: String,
    /// Automatically suggest an answer when "Mereka" asks a question.
    pub auto_assist: bool,
    pub active_preset: String,
    pub presets: Vec<Preset>,
    /// Ask the user to confirm participants were told about recording (UU PDP).
    pub consent_reminder: bool,
    /// Keep the WAV recordings of each meeting.
    pub keep_audio: bool,
    /// Conversation language: "id", "en" or "auto" (transcription + AI answers).
    pub language: String,
    /// Microphone device name (`None` = system default).
    pub mic_device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::ClaudeCli {
                model: "sonnet".into(),
                binary: None,
            },
            stt_model: crate::models::DEFAULT_MODEL.into(),
            vocabulary: String::new(),
            register: "formal".into(),
            auto_assist: true,
            active_preset: "rapat".into(),
            presets: builtin_presets(),
            consent_reminder: true,
            keep_audio: true,
            language: "id".into(),
            mic_device: None,
        }
    }
}

impl Settings {
    pub fn preset(&self) -> Preset {
        self.presets
            .iter()
            .find(|p| p.id == self.active_preset)
            .or(self.presets.first())
            .cloned()
            .unwrap_or_else(|| builtin_presets().remove(0))
    }
}

pub fn builtin_presets() -> Vec<Preset> {
    let p = |id: &str, name: &str, prompt: &str| Preset {
        id: id.into(),
        name: name.into(),
        system_prompt: prompt.into(),
        context: String::new(),
        builtin: true,
    };
    vec![
        p(
            "rapat",
            "Rapat kerja",
            "Kamu adalah asisten rapat yang tajam. Bantu pengguna memberi tanggapan yang jelas, \
             berbasis data, dan mendorong keputusan. Tandai keputusan, action item, dan hal yang \
             belum jelas.",
        ),
        p(
            "interview",
            "Interview kerja (saya kandidat)",
            "Pengguna adalah kandidat dalam wawancara kerja. Bantu menjawab pertanyaan HRD/user \
             dengan percaya diri dan jujur, gunakan metode STAR (Situasi, Tugas, Aksi, Hasil) \
             untuk pertanyaan perilaku, dan ambil contoh dari CV/pengalaman di konteks. Jangan \
             mengarang pengalaman yang tidak ada di konteks; jika tidak ada, beri kerangka jawaban.",
        ),
        p(
            "interviewer",
            "Interview kerja (saya pewawancara)",
            "Pengguna adalah PEWAWANCARA; \"Mereka\" adalah kandidat. Bantu pengguna menggali kandidat: \
             sarankan pertanyaan lanjutan yang tajam berdasarkan jawaban terakhir (minta contoh konkret, \
             angka, peran pribadi vs tim), tandai jawaban yang kabur, tidak konsisten, atau tidak sesuai \
             CV/deskripsi pekerjaan di konteks, dan nilai jawaban secara singkat (kuat / cukup / lemah \
             beserta alasannya). Saat diminta \"Bantu jawab\", berikan apa yang sebaiknya pewawancara \
             ucapkan berikutnya. Tetap objektif dan hindari pertanyaan diskriminatif (usia, agama, status \
             pernikahan, rencana punya anak).",
        ),
        p(
            "klien",
            "Diskusi klien / konsultasi",
            "Pengguna adalah konsultan yang berdiskusi dengan klien (mis. scoping proyek). Bantu \
             menjelaskan dan membela keputusan pengguna dengan alasan teknis yang jelas, jawab \
             pertanyaan dan kekhawatiran klien dengan jujur (akui risiko residual bila ada, tawarkan \
             mitigasi atau opsi tambahan), dan pastikan kesepakatan serta asumsi tercatat. Ikuti posisi \
             dan catatan pengguna di konteks; jangan mengubah posisi pengguna tanpa alasan kuat.",
        ),
        p(
            "webinar",
            "Webinar / presentasi",
            "Pengguna sedang mengikuti atau membawakan webinar. Bantu menjawab pertanyaan peserta \
             dengan ringkas dan akurat, dan sarankan pertanyaan yang bagus untuk diajukan.",
        ),
        p(
            "sales",
            "Sales call",
            "Pengguna sedang melakukan sales call. Bantu menangani keberatan, menggali kebutuhan \
             dengan pertanyaan terbuka, dan menjelaskan nilai produk dari konteks. Jangan menjanjikan \
             hal yang tidak ada di konteks.",
        ),
    ]
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let mut s: Settings = path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // Make sure built-in presets exist (new ones may ship in updates).
    for b in builtin_presets() {
        if !s.presets.iter().any(|p| p.id == b.id) {
            s.presets.push(b);
        }
    }
    s
}

fn save(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let p = path(app).ok_or("app data dir tidak tersedia")?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, serde_json::to_string_pretty(s).unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    save(&app, &settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    crate::assist::prewarm(&app);
    Ok(settings)
}

#[tauri::command]
pub fn reset_preset(state: State<'_, AppState>, id: String) -> Option<Preset> {
    let builtin = builtin_presets().into_iter().find(|p| p.id == id)?;
    let s = state.settings.lock().unwrap();
    let ctx = s
        .presets
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.context.clone())
        .unwrap_or_default();
    Some(Preset {
        context: ctx,
        ..builtin
    })
}

// ---------------------------------------------------------------- API keys (OS keychain)

const SERVICE: &str = "id.bicara.desktop";

pub fn api_key(account: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, account)
        .ok()?
        .get_password()
        .ok()
}

#[tauri::command]
pub fn set_api_key(account: String, key: String) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, &account).map_err(|e| e.to_string())?;
    if key.is_empty() {
        let _ = e.delete_credential();
        Ok(())
    } else {
        e.set_password(&key).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn has_api_key(account: String) -> bool {
    api_key(&account).is_some_and(|k| !k.is_empty())
}

#[tauri::command]
pub async fn detect_clis() -> bicara_llm::CliStatus {
    tauri::async_runtime::spawn_blocking(|| {
        tauri::async_runtime::block_on(bicara_llm::detect_clis())
    })
    .await
    .unwrap_or(bicara_llm::CliStatus {
        claude: None,
        codex: None,
    })
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    bicara_audio::list_input_devices()
}
