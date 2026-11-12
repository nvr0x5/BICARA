//! Stealth mode: hide BICARA windows from screen sharing / recording, plus a panic hotkey.
//!
//! * Windows: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` via Tauri content protection,
//!   and hidden from the taskbar.
//! * macOS: `NSWindow.sharingType = .none` via Tauri content protection, and the app switches to
//!   the "accessory" activation policy (no Dock icon, not in Cmd-Tab).

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::AppState;

fn panic_shortcut() -> Shortcut {
    #[cfg(target_os = "macos")]
    let mods = Modifiers::SUPER | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    Shortcut::new(Some(mods), Code::KeyH)
}

pub fn panic_hotkey_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() == ShortcutState::Pressed && *shortcut == panic_shortcut() {
                toggle_all(app);
            }
        })
        .build()
}

pub fn register_panic_hotkey<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.global_shortcut()
        .register(panic_shortcut())
        .map_err(|e| tauri::Error::Anyhow(e.into()))
}

fn toggle_all<R: Runtime>(app: &AppHandle<R>) {
    let windows = app.webview_windows();
    let any_visible = windows.values().any(|w| w.is_visible().unwrap_or(false));
    for w in windows.values() {
        let _ = if any_visible { w.hide() } else { w.show() };
    }
}

#[tauri::command]
pub fn toggle_visibility(app: AppHandle) {
    toggle_all(&app);
}

#[tauri::command]
pub fn panic_hotkey_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘ + ⇧ + H"
    } else {
        "Ctrl + Shift + H"
    }
}

#[tauri::command]
pub fn get_stealth(state: State<'_, AppState>) -> bool {
    *state.stealth.lock().unwrap()
}

#[tauri::command]
pub fn set_stealth(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    for w in app.webview_windows().values() {
        w.set_content_protected(enabled)
            .map_err(|e| e.to_string())?;
        w.set_skip_taskbar(enabled).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        let policy = if enabled {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| e.to_string())?;
        // Switching policy can deactivate the app; keep our windows on screen.
        for w in app.webview_windows().values() {
            let _ = w.show();
        }
    }
    *state.stealth.lock().unwrap() = enabled;
    Ok(enabled)
}
