mod assist;
mod capture;
mod models;
mod settings;
mod stealth;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::Manager;

type SttSlot = Option<(String, String, Arc<bicara_stt::LiveTranscriber>)>;

pub struct AppState {
    pub session: Mutex<Option<capture::Session>>,
    pub stealth: Mutex<bool>,
    pub settings: Mutex<settings::Settings>,
    /// Loaded whisper model: (model file, vocabulary, transcriber).
    pub stt: Mutex<SttSlot>,
    pub transcript: Mutex<Vec<bicara_stt::Utterance>>,
    pub last_question: Mutex<Option<String>>,
    pub last_auto_assist: Mutex<Option<Instant>>,
    pub llm: Arc<bicara_llm::Llm>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(stealth::hotkey_plugin())
        .setup(|app| {
            let handle = app.handle();
            app.manage(AppState {
                session: Mutex::new(None),
                stealth: Mutex::new(false),
                settings: Mutex::new(settings::load(handle)),
                stt: Mutex::new(None),
                transcript: Mutex::new(Vec::new()),
                last_question: Mutex::new(None),
                last_auto_assist: Mutex::new(None),
                llm: Arc::new(bicara_llm::Llm::new()),
            });
            stealth::register_hotkeys(handle)?;
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(dir.join("recordings"));
            }
            models::preload(handle);
            assist::prewarm(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture::list_audio_apps,
            capture::start_capture,
            capture::stop_capture,
            capture::recordings_dir,
            stealth::set_stealth,
            stealth::get_stealth,
            stealth::toggle_visibility,
            stealth::hotkey_labels,
            settings::get_settings,
            settings::save_settings,
            settings::reset_preset,
            settings::set_api_key,
            settings::has_api_key,
            settings::detect_clis,
            models::list_models,
            models::download_model,
            models::stt_loaded,
            assist::ai_action,
            assist::get_transcript,
            assist::clear_transcript,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
