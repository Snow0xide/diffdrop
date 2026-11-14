//! Commands called by the frontend.

use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::i18n::{self, t};
use crate::menu::{self, MenuState};
use crate::session::{self, Side, Snapshot};

/// Whether the translucent window background is active, set at startup.
pub struct Effects(pub bool);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Init {
    os: &'static str,
    lang: &'static str,
    effects: bool,
    session: Snapshot,
}

#[tauri::command]
pub fn init(app: AppHandle) -> Init {
    Init {
        os: std::env::consts::OS,
        lang: i18n::lang(),
        effects: app.try_state::<Effects>().is_some_and(|e| e.0),
        session: session::snapshot(&app),
    }
}

/// The latest diff as JSON, sent as is to avoid serializing it twice.
#[tauri::command]
pub fn get_diff(app: AppHandle) -> Response {
    match session::diff_json(&app) {
        Some(json) => Response::new(String::clone(&json)),
        None => Response::new("null".to_owned()),
    }
}

#[tauri::command]
pub fn open_paths(app: AppHandle, paths: Vec<PathBuf>, side: Option<Side>) {
    session::open(&app, paths, side);
}

#[tauri::command]
pub fn choose(app: AppHandle, side: Option<Side>) {
    choose_files(&app, side);
}

#[tauri::command]
pub fn swap(app: AppHandle) {
    session::swap(&app);
}

#[tauri::command]
pub fn reset(app: AppHandle) {
    session::reset(&app);
}

#[tauri::command]
pub fn set_ignore_whitespace(app: AppHandle, value: bool) {
    session::set_ignore_whitespace(&app, value);
}

#[tauri::command]
pub fn sync_menu(app: AppHandle, state: MenuState) {
    menu::sync(&app, state);
}

/// System file picker, shown as a sheet on the main window. With a side it
/// picks one file for that zone; without, one or two files.
pub fn choose_files(app: &AppHandle, side: Option<Side>) {
    let title = match side {
        Some(Side::A) => t("dialog.pickA"),
        Some(Side::B) => t("dialog.pickB"),
        None => t("dialog.pickAny"),
    };
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let app = app.clone();
    match side {
        Some(side) => dialog.pick_file(move |file| {
            if let Some(path) = file.and_then(|f| f.into_path().ok()) {
                session::open(&app, vec![path], Some(side));
            }
        }),
        None => dialog.pick_files(move |files| {
            let paths: Vec<PathBuf> = files
                .unwrap_or_default()
                .into_iter()
                .filter_map(|f| f.into_path().ok())
                .collect();
            session::open(&app, paths, None);
        }),
    }
}
