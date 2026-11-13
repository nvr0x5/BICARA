mod assist;
mod capture;
mod meeting;
mod menu;
mod models;
mod settings;
mod stealth;
mod store;

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
    pub store: Mutex<store::Store>,
    /// Meeting currently being recorded.
    pub current_meeting: Mutex<Option<i64>>,
    /// Mic muted: "Saya" audio is replaced by silence (not recorded, not transcribed).
    pub mic_muted: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(stealth::hotkey_plugin())
        .setup(|app| {
            let handle = app.handle();
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = store::Store::open(&data_dir.join("bicara.db"))
                .map_err(|e| tauri::Error::Anyhow(e.into()))?;
            app.manage(AppState {
                store: Mutex::new(store),
                current_meeting: Mutex::new(None),
                mic_muted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            menu::install(handle);
            #[cfg(debug_assertions)]
            if std::env::var_os("BICARA_DEVTOOLS").is_some() {
                if let Some(w) = app.get_webview_window("main") {
                    w.open_devtools();
                }
            }
            models::preload(handle);
            assist::prewarm(handle);
            Ok(())
        })
        .on_menu_event(|app, event| menu::on_event(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            capture::list_audio_apps,
            capture::start_capture,
            capture::stop_capture,
            capture::set_mic_muted,
            capture::recordings_dir,
            menu::open_settings_window,
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
            settings::list_microphones,
            settings::list_provider_models,
            settings::test_provider,
            models::list_models,
            models::download_model,
            models::stt_loaded,
            assist::ai_action,
            assist::get_transcript,
            assist::clear_transcript,
            meeting::create_meeting,
            meeting::add_meeting_files,
            meeting::remove_meeting_document,
            meeting::supported_document_extensions,
            meeting::start_meeting,
            meeting::end_meeting,
            meeting::current_meeting,
            meeting::generate_notulen,
            meeting::save_notulen,
            meeting::interview_questions,
            meeting::prepare_interview,
            meeting::list_meetings,
            meeting::search_meetings,
            meeting::get_meeting,
            meeting::rename_meeting,
            meeting::delete_meeting,
            meeting::ask_meeting,
            meeting::export_meeting,
            meeting::whatsapp_share_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
