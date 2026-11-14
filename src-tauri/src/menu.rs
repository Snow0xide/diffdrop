//! Native menu bar, macOS only: on Windows the same shortcuts are handled by
//! the frontend, so no menu bar sits between the title bar and the toolbar.

use serde::Deserialize;
#[cfg(target_os = "macos")]
use tauri::Emitter;
use tauri::menu::CheckMenuItem;
#[cfg(target_os = "macos")]
use tauri::menu::{
    AboutMetadata, CheckMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItemBuilder,
    SubmenuBuilder,
};
use tauri::{AppHandle, Manager, Wry};

#[cfg(target_os = "macos")]
use crate::{i18n::t, session};

/// Items whose check mark or availability follows the window state.
pub struct MenuItems {
    split: CheckMenuItem<Wry>,
    unified: CheckMenuItem<Wry>,
    ignore_whitespace: CheckMenuItem<Wry>,
    only_changes: CheckMenuItem<Wry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuState {
    unified: bool,
    only_changes: bool,
    ignore_whitespace: bool,
}

#[cfg(target_os = "macos")]
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let check = |id: &str, key: &str, checked: bool| {
        CheckMenuItemBuilder::with_id(id, t(key))
            .checked(checked)
            .build(app)
    };
    let split = check("view-split", "menu.split", true)?;
    let unified = check("view-unified", "menu.unified", false)?;
    let ignore_whitespace = check("ignore-whitespace", "menu.ignoreWhitespace", false)?;
    let only_changes = check("only-changes", "menu.onlyChanges", false)?;

    let item = |id: &str, key: &str, accelerator: Option<&str>| {
        let mut builder = MenuItemBuilder::with_id(id, t(key));
        if let Some(accelerator) = accelerator {
            builder = builder.accelerator(accelerator);
        }
        builder.build(app)
    };

    let about = AboutMetadata {
        name: Some("diffdrop".into()),
        version: Some(app.package_info().version.to_string()),
        license: Some("MIT".into()),
        ..Default::default()
    };
    let app_menu = SubmenuBuilder::new(app, "diffdrop")
        .about_with_text(t("menu.about"), Some(about))
        .separator()
        .services_with_text(t("menu.services"))
        .separator()
        .hide_with_text(t("menu.hide"))
        .hide_others_with_text(t("menu.hideOthers"))
        .show_all_with_text(t("menu.showAll"))
        .separator()
        .quit_with_text(t("menu.quit"))
        .build()?;

    let file = SubmenuBuilder::new(app, t("menu.file"))
        .item(&item("open", "menu.open", Some("CmdOrCtrl+O"))?)
        .item(&item("reset", "menu.reset", None)?)
        .separator()
        .close_window_with_text(t("menu.closeWindow"))
        .build()?;

    let edit = SubmenuBuilder::new(app, t("menu.edit"))
        .undo_with_text(t("menu.undo"))
        .redo_with_text(t("menu.redo"))
        .separator()
        .cut_with_text(t("menu.cut"))
        .copy_with_text(t("menu.copy"))
        .paste_with_text(t("menu.paste"))
        .select_all_with_text(t("menu.selectAll"))
        .build()?;

    let view = SubmenuBuilder::new(app, t("menu.view"))
        .item(&split)
        .item(&unified)
        .separator()
        .item(&ignore_whitespace)
        .item(&only_changes)
        .separator()
        .item(&item("swap", "menu.swap", Some("CmdOrCtrl+Shift+S"))?)
        .separator()
        .item(&item("next-change", "menu.next", Some("CmdOrCtrl+Down"))?)
        .item(&item("prev-change", "menu.prev", Some("CmdOrCtrl+Up"))?)
        .separator()
        .fullscreen_with_text(t("menu.fullscreen"))
        .build()?;

    let window = SubmenuBuilder::new(app, t("menu.window"))
        .minimize_with_text(t("menu.minimize"))
        .maximize_with_text(t("menu.zoom"))
        .separator()
        .bring_all_to_front_with_text(t("menu.bringAllToFront"))
        .build()?;

    let help = SubmenuBuilder::new(app, t("menu.help")).build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &view, &window, &help])
        .build()?;

    window.set_as_windows_menu_for_nsapp()?;
    help.set_as_help_menu_for_nsapp()?;
    app.manage(MenuItems {
        split,
        unified,
        ignore_whitespace,
        only_changes,
    });
    Ok(menu)
}

#[cfg(target_os = "macos")]
pub fn on_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => crate::commands::choose_files(app, None),
        "reset" => session::reset(app),
        "swap" => session::swap(app),
        "ignore-whitespace" => {
            let value = !session::ignore_whitespace(app);
            session::set_ignore_whitespace(app, value);
        }
        // Pure view state lives in the frontend, which then calls `sync`.
        id @ ("view-split" | "view-unified" | "only-changes" | "next-change" | "prev-change") => {
            let _ = app.emit("menu", id);
        }
        _ => {}
    }
}

/// Puts the check marks back in line with the window, including after a
/// click, since check items toggle themselves.
pub fn sync(app: &AppHandle, state: MenuState) {
    let Some(items) = app.try_state::<MenuItems>() else {
        return;
    };
    let _ = items.split.set_checked(!state.unified);
    let _ = items.unified.set_checked(state.unified);
    let _ = items.ignore_whitespace.set_checked(state.ignore_whitespace);
    let _ = items.only_changes.set_checked(state.only_changes);
}
