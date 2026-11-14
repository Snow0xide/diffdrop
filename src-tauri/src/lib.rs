mod appearance;
mod commands;
mod i18n;
mod menu;
mod session;
mod watcher;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_cli::{CliExt, Matches};

/// `diffdrop fileA fileB`: positional arguments, relative to `cwd`.
fn files_from_matches(matches: &Matches, cwd: &Path) -> Vec<PathBuf> {
    ["fileA", "fileB"]
        .iter()
        .filter_map(|name| matches.args.get(*name)?.value.as_str())
        .map(|arg| {
            // Tidies `./file` into a clean path for display.
            let path = cwd.join(arg);
            std::path::absolute(&path).unwrap_or(path)
        })
        .collect()
}

fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("diffdrop")
        .inner_size(1180.0, 760.0)
        .min_inner_size(720.0, 460.0)
        .transparent(true);
    // Title bar merged with the toolbar. The buttons sit 9pt above the bottom
    // of their 14 + y tall container, so y = 25 centres them in the 46px
    // toolbar (see toolbar.css).
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(18.0, 25.0));
    let window = builder.build()?;

    app.manage(commands::Effects(appearance::apply(&window)));
    // Mica picks light or dark when applied, so apply it again on change.
    #[cfg(target_os = "windows")]
    {
        let target = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::ThemeChanged(_) = event {
                appearance::apply(&target);
            }
        });
    }
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default()
        // Must come first: a second launch hands its arguments over and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Ok(matches) = app.cli().matches_from(argv) {
                session::open(app, files_from_matches(&matches, Path::new(&cwd)), None);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(session::Session::default())
        .invoke_handler(tauri::generate_handler![
            commands::init,
            commands::get_diff,
            commands::open_paths,
            commands::choose,
            commands::swap,
            commands::reset,
            commands::set_ignore_whitespace,
            commands::sync_menu,
        ])
        .setup(|app| {
            let handle = app.handle();
            // Invalid arguments (for example the -psn_ flag some macOS
            // versions pass) are ignored rather than stopping the app.
            if let Ok(matches) = app.cli().matches() {
                if let Some(help) = matches.args.get("help").and_then(|a| a.value.as_str()) {
                    println!("{help}");
                    std::process::exit(0);
                }
                if matches.args.contains_key("version") {
                    println!("diffdrop {}", app.package_info().version);
                    std::process::exit(0);
                }
                let cwd = std::env::current_dir().unwrap_or_default();
                session::open(handle, files_from_matches(&matches, &cwd), None);
            }
            create_window(handle)?;
            Ok(())
        });

    #[cfg(target_os = "macos")]
    let builder = builder.menu(menu::build).on_menu_event(menu::on_event);

    builder
        .build(tauri::generate_context!())
        .expect("error while building the application")
        .run(|_app, _event| {
            // Files dropped on the Dock icon or opened from Finder.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                let paths = urls
                    .into_iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .collect();
                session::open(_app, paths, None);
            }
        });
}
