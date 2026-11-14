//! Reloads the comparison when one of the open files changes on disk.
//!
//! The parent folders are watched rather than the files themselves: many
//! editors save by writing a temporary file and renaming it over the original,
//! which would end a watch placed on the old file.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::AppHandle;

/// Saves often arrive as a burst of events; wait for this much quiet.
const SETTLE: Duration = Duration::from_millis(150);

pub struct FileWatcher {
    files: [Option<PathBuf>; 2],
    // Dropping the watcher closes the channel, which ends the thread.
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn files(&self) -> &[Option<PathBuf>; 2] {
        &self.files
    }

    pub fn new(app: &AppHandle, files: &[Option<PathBuf>; 2]) -> Option<Self> {
        // (canonical parent folder, file name) of each watched file.
        let targets: HashSet<(PathBuf, OsString)> = files
            .iter()
            .flatten()
            .filter_map(|path| split(path))
            .collect();
        if targets.is_empty() {
            return None;
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })
        .ok()?;
        let folders: HashSet<&PathBuf> = targets.iter().map(|(folder, _)| folder).collect();
        for folder in folders {
            watcher.watch(folder, RecursiveMode::NonRecursive).ok()?;
        }

        let app = app.clone();
        std::thread::spawn(move || {
            let relevant = |event: &notify::Result<notify::Event>| match event {
                Ok(event) => {
                    // Opening a file on macOS updates its metadata (last
                    // used date); only content changes matter here.
                    !matches!(
                        event.kind,
                        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
                    ) && event
                        .paths
                        .iter()
                        .any(|p| split(p).is_some_and(|t| targets.contains(&t)))
                }
                Err(_) => false,
            };
            while let Ok(event) = rx.recv() {
                if !relevant(&event) {
                    continue;
                }
                loop {
                    match rx.recv_timeout(SETTLE) {
                        Ok(_) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                crate::session::reload(&app);
            }
        });

        Some(FileWatcher {
            files: files.clone(),
            _watcher: watcher,
        })
    }
}

/// Canonical parent folder and file name. Paths reported by the system can
/// differ from the ones we opened (symlinks such as /var on macOS), so both
/// sides are compared in canonical form.
fn split(path: &Path) -> Option<(PathBuf, OsString)> {
    let name = path.file_name()?.to_owned();
    let folder = path.parent()?.canonicalize().ok()?;
    Some((folder, name))
}
