//! Stealth mode: hide BICARA windows from screen sharing / recording, plus a panic hotkey.
//!
//! * Windows: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` via Tauri content protection,
//!   and hidden from the taskbar.
//! * macOS: `NSWindow.sharingType = .none` via Tauri content protection, and the app switches to
//!   the "accessory" activation policy (no Dock icon, not in Cmd-Tab).

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::AppState;

fn mods() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::SUPER | Modifiers::SHIFT
    } else {
        Modifiers::CONTROL | Modifiers::SHIFT
    }
}

/// ⌘⇧H / Ctrl+Shift+H — hide or show every BICARA window.
fn panic_shortcut() -> Shortcut {
    Shortcut::new(Some(mods()), Code::KeyH)
}

/// ⌘⇧J / Ctrl+Shift+J — "Bantu jawab": suggest an answer to the last question.
fn assist_shortcut() -> Shortcut {
    Shortcut::new(Some(mods()), Code::KeyJ)
}

pub fn hotkey_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if *shortcut == panic_shortcut() {
                toggle_all(app);
            } else if *shortcut == assist_shortcut() {
                crate::assist::run(app.clone(), crate::assist::Action::Assist, None);
            }
        })
        .build()
}

pub fn register_hotkeys<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    for s in [panic_shortcut(), assist_shortcut()] {
        app.global_shortcut()
            .register(s)
            .map_err(|e| tauri::Error::Anyhow(e.into()))?;
    }
    Ok(())
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

#[derive(serde::Serialize)]
pub struct HotkeyLabels {
    panic: &'static str,
    assist: &'static str,
}

#[tauri::command]
pub fn hotkey_labels() -> HotkeyLabels {
    if cfg!(target_os = "macos") {
        HotkeyLabels {
            panic: "⌘⇧H",
            assist: "⌘⇧J",
        }
    } else {
        HotkeyLabels {
            panic: "Ctrl+Shift+H",
            assist: "Ctrl+Shift+J",
        }
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
