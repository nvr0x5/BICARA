mod capture;
mod stealth;

use std::sync::Mutex;

use tauri::Manager;

pub struct AppState {
    pub session: Mutex<Option<capture::Session>>,
    pub stealth: Mutex<bool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(stealth::panic_hotkey_plugin())
        .manage(AppState {
            session: Mutex::new(None),
            stealth: Mutex::new(false),
        })
        .setup(|app| {
            stealth::register_panic_hotkey(app.handle())?;
            if let Some(dir) = app.path().app_data_dir().ok() {
                let _ = std::fs::create_dir_all(dir.join("recordings"));
            }
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
            stealth::panic_hotkey_label,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
