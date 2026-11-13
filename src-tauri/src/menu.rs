//! Native menu bar (macOS) / window menu (Windows), plus the Preferences window.
//! Menu commands the UI handles are forwarded as a `menu` event carrying the item id.

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Wry};

use crate::AppState;

fn build(app: &AppHandle, en: bool) -> tauri::Result<Menu<Wry>> {
    let tr = |id: &'static str, en_s: &'static str| if en { en_s } else { id };
    let item = |id: &str, label: &str, accel: Option<&str>| {
        let b = MenuItemBuilder::with_id(id, label);
        match accel {
            Some(a) => b.accelerator(a).build(app),
            None => b.build(app),
        }
    };

    let settings = item("settings", tr("Pengaturan…", "Settings…"), Some("CmdOrCtrl+,"))?;
    let app_menu = SubmenuBuilder::new(app, "BICARA")
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, tr("Berkas", "File"))
        .item(&item("new", tr("Meeting baru", "New meeting"), Some("CmdOrCtrl+N"))?)
        .item(&item("history", tr("Semua riwayat", "All history"), Some("CmdOrCtrl+Shift+R"))?)
        .separator()
        .close_window()
        .build()?;

    let edit = SubmenuBuilder::new(app, tr("Edit", "Edit"))
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // ⌘⇧J is a global hotkey (works while Zoom is focused), so it is only shown in the label.
    let meeting = SubmenuBuilder::new(app, "Meeting")
        .item(&item("assist", tr("Bantu jawab  (⌘⇧J)", "Help me answer  (⌘⇧J)"), None)?)
        .item(&item("followup", tr("Pertanyaan lanjutan", "Follow-up questions"), None)?)
        .item(&item("summary", tr("Ringkas", "Summarize"), None)?)
        .separator()
        .item(&item("end", tr("Akhiri meeting", "End meeting"), Some("CmdOrCtrl+Shift+E"))?)
        .build()?;

    let view = SubmenuBuilder::new(app, tr("Tampilan", "View"))
        .item(&item("toggle-sidebar", tr("Tampilkan/sembunyikan sidebar", "Show/hide sidebar"), Some("Ctrl+CmdOrCtrl+S"))?)
        .item(&item("toggle-inspector", tr("Tampilkan/sembunyikan asisten", "Show/hide assistant"), Some("Alt+CmdOrCtrl+I"))?)
        .item(&item("overlay", tr("Tampilkan overlay", "Show overlay"), None)?)
        .separator()
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, tr("Jendela", "Window"))
        .minimize()
        .maximize()
        .build()?;

    let mut b = MenuBuilder::new(app);
    if cfg!(target_os = "macos") {
        b = b.item(&app_menu);
    }
    b.items(&[&file, &edit, &meeting, &view, &window]).build()
}

/// Installs (or re-installs after a language change) the menu bar.
pub fn install(app: &AppHandle) {
    let en = app.state::<AppState>().settings.lock().unwrap().ui_language == "en";
    match build(app, en) {
        Ok(m) => {
            if cfg!(target_os = "macos") {
                let _ = app.set_menu(m);
            }
            // On Windows the UI's own navigation replaces a window menu bar.
        }
        Err(e) => log::error!("[bicara] menu: {e}"),
    }
}

pub fn on_event(app: &AppHandle, id: &str) {
    match id {
        "settings" => open_settings(app),
        "overlay" => {
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        "assist" | "followup" | "summary" => {
            let action = match id {
                "assist" => crate::assist::Action::Assist,
                "followup" => crate::assist::Action::Followup,
                _ => crate::assist::Action::Summary,
            };
            crate::assist::run(app.clone(), action, None);
        }
        other => {
            let _ = app.emit("menu", other);
        }
    }
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    open_settings(&app);
}

/// Opens (or focuses) the Preferences window — the standard macOS pattern.
pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let en = app.state::<AppState>().settings.lock().unwrap().ui_language == "en";
    let stealth = *app.state::<AppState>().stealth.lock().unwrap();
    match WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html?view=settings".into()))
        .title(if en { "Settings" } else { "Pengaturan" })
        .inner_size(720.0, 640.0)
        .min_inner_size(620.0, 480.0)
        .content_protected(stealth)
        .build()
    {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => log::error!("[bicara] settings window: {e}"),
    }
}
