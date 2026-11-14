//! The two files being compared and the latest result. Everything that can
//! load a file (drops, dialogs, command line, Dock, second instance, file
//! watcher) goes through here, and the frontend is told with a `session` event.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use diff_core::{Decoded, Options};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::watcher::FileWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    A,
    B,
}

impl Side {
    fn index(self) -> usize {
        match self {
            Side::A => 0,
            Side::B => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FileError {
    Binary,
    Folder,
    Unreadable { reason: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    /// Fewer than two files.
    #[default]
    Idle,
    Computing,
    Ready,
    /// At least one file could not be loaded as text.
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileView {
    path: String,
    name: String,
}

/// What the frontend needs to draw everything except the diff itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    files: [Option<FileView>; 2],
    errors: [Option<FileError>; 2],
    ignore_whitespace: bool,
    status: Status,
    /// Generation of the diff available from `get_diff`, if any.
    ready: Option<u64>,
    /// Increases with every change, so the frontend can drop stale snapshots.
    seq: u64,
}

#[derive(Default)]
pub struct Session(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    files: [Option<PathBuf>; 2],
    errors: [Option<FileError>; 2],
    options: Options,
    status: Status,
    generation: u64,
    /// Serialized result, kept until a newer one replaces it.
    diff: Option<(u64, Arc<String>)>,
    watcher: Option<FileWatcher>,
    seq: u64,
}

fn lock(app: &AppHandle) -> MutexGuard<'_, Inner> {
    app.state::<Session>()
        .inner()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Two or more paths fill A and B in order. A single path goes to `side`, or
/// to the first empty zone; if both are taken it starts a new comparison.
pub fn open(app: &AppHandle, paths: Vec<PathBuf>, side: Option<Side>) {
    let mut inner = lock(app);
    let mut paths = paths.into_iter();
    let (Some(first), second) = (paths.next(), paths.next()) else {
        return;
    };
    match (second, side) {
        (Some(second), _) => inner.files = [Some(first), Some(second)],
        (None, Some(side)) => inner.files[side.index()] = Some(first),
        (None, None) => match inner.files.iter().position(Option::is_none) {
            Some(empty) => inner.files[empty] = Some(first),
            None => inner.files = [Some(first), None],
        },
    }
    inner.errors = [None, None];
    refresh(app, &mut inner);
}

pub fn swap(app: &AppHandle) {
    let mut inner = lock(app);
    inner.files.swap(0, 1);
    inner.errors.swap(0, 1);
    refresh(app, &mut inner);
}

pub fn reset(app: &AppHandle) {
    let mut inner = lock(app);
    inner.files = [None, None];
    inner.errors = [None, None];
    refresh(app, &mut inner);
}

pub fn set_ignore_whitespace(app: &AppHandle, value: bool) {
    let mut inner = lock(app);
    if inner.options.ignore_whitespace != value {
        inner.options.ignore_whitespace = value;
        refresh(app, &mut inner);
    }
}

/// Read by the macOS menu bar, the only native control for this option.
#[cfg(target_os = "macos")]
pub fn ignore_whitespace(app: &AppHandle) -> bool {
    lock(app).options.ignore_whitespace
}

/// Called by the watcher when one of the files changed on disk.
pub fn reload(app: &AppHandle) {
    let mut inner = lock(app);
    refresh(app, &mut inner);
}

pub fn snapshot(app: &AppHandle) -> Snapshot {
    snapshot_of(&lock(app))
}

pub fn diff_json(app: &AppHandle) -> Option<Arc<String>> {
    lock(app).diff.as_ref().map(|(_, json)| json.clone())
}

fn snapshot_of(inner: &Inner) -> Snapshot {
    let view = |path: &Option<PathBuf>| {
        path.as_ref().map(|p| FileView {
            path: p.to_string_lossy().into_owned(),
            name: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
        })
    };
    Snapshot {
        files: [view(&inner.files[0]), view(&inner.files[1])],
        errors: inner.errors.clone(),
        ignore_whitespace: inner.options.ignore_whitespace,
        status: inner.status,
        ready: inner.diff.as_ref().map(|(generation, _)| *generation),
        seq: inner.seq,
    }
}

fn emit(app: &AppHandle, inner: &mut Inner) {
    inner.seq += 1;
    let _ = app.emit("session", snapshot_of(inner));
}

/// Starts loading and diffing the current files on a worker thread. The old
/// result stays available until the new one is ready, so the view does not
/// flash when a file is saved.
fn refresh(app: &AppHandle, inner: &mut Inner) {
    inner.generation += 1;
    let generation = inner.generation;

    let watched = inner.watcher.as_ref().map(FileWatcher::files);
    if watched != Some(&inner.files) {
        inner.watcher = FileWatcher::new(app, &inner.files);
    }

    if inner.files.iter().all(Option::is_none) {
        inner.status = Status::Idle;
        inner.diff = None;
        emit(app, inner);
        return;
    }
    // With a single file the worker only checks that it can be read as text.
    if inner.files.iter().all(Option::is_some) {
        inner.status = Status::Computing;
    } else {
        inner.status = Status::Idle;
        inner.diff = None;
    }
    emit(app, inner);

    let files = inner.files.clone();
    let options = inner.options;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (errors, diff) = compute(generation, &files, options);
        let mut inner = lock(&app);
        if inner.generation != generation {
            return; // a newer request is on its way
        }
        let both = files.iter().all(Option::is_some);
        inner.status = match (&diff, both) {
            (Some(_), _) => Status::Ready,
            (None, true) => Status::Failed,
            (None, false) => Status::Idle,
        };
        inner.errors = errors;
        inner.diff = diff.map(|json| (generation, Arc::new(json)));
        emit(&app, &mut inner);
    });
}

fn load(path: &Path) -> Result<String, FileError> {
    if path.is_dir() {
        return Err(FileError::Folder);
    }
    let bytes = std::fs::read(path).map_err(|e| FileError::Unreadable {
        reason: e.to_string(),
    })?;
    match diff_core::decode(&bytes) {
        Decoded::Text { text, .. } => Ok(text),
        Decoded::Binary => Err(FileError::Binary),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload<'a> {
    generation: u64,
    old_lines: Vec<&'a str>,
    new_lines: Vec<&'a str>,
    ops: &'a [diff_core::Op],
    stats: &'a diff_core::Stats,
}

type Computed = ([Option<FileError>; 2], Option<String>);

fn compute(generation: u64, files: &[Option<PathBuf>; 2], options: Options) -> Computed {
    let loaded = [files[0].as_deref().map(load), files[1].as_deref().map(load)];
    let errors = [
        loaded[0].as_ref().and_then(|r| r.as_ref().err().cloned()),
        loaded[1].as_ref().and_then(|r| r.as_ref().err().cloned()),
    ];
    let (Some(Ok(old)), Some(Ok(new))) = (&loaded[0], &loaded[1]) else {
        return (errors, None);
    };

    let diff = diff_core::diff(old, new, options);
    let payload = Payload {
        generation,
        old_lines: diff.old_lines.iter().map(|l| l.text(old)).collect(),
        new_lines: diff.new_lines.iter().map(|l| l.text(new)).collect(),
        ops: &diff.ops,
        stats: &diff.stats,
    };
    let json = serde_json::to_string(&payload).expect("diff payload serializes");
    (errors, Some(json))
}
