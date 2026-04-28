// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::Watcher as _;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileChangeKind {
    Created,
    Changed,
    Deleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    watched_files: Arc<Mutex<HashSet<PathBuf>>>,
    watched_dirs: HashSet<PathBuf>,
}

impl FileWatcher {
    pub fn start(mut on_event: impl FnMut(WatchEvent) + Send + 'static) -> notify::Result<Self> {
        let watched_files = Arc::new(Mutex::new(HashSet::new()));
        let callback_files = watched_files.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            forward_event(event, &callback_files, &mut on_event);
        })?;

        Ok(Self { watcher, watched_files, watched_dirs: HashSet::new() })
    }

    pub fn update_watched_paths<I>(&mut self, paths: I) -> notify::Result<()>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let watched_files = paths
            .into_iter()
            .map(|path| i_slint_compiler::pathutils::clean_path(&path))
            .collect::<HashSet<_>>();
        let watched_dirs = watched_files
            .iter()
            .filter_map(|path| watch_directory_for_path(path))
            .collect::<HashSet<_>>();

        for dir in self.watched_dirs.difference(&watched_dirs) {
            self.watcher.unwatch(dir)?;
        }

        for dir in watched_dirs.difference(&self.watched_dirs) {
            self.watcher.watch(dir, watch_mode())?;
        }

        *self.watched_files.lock().unwrap() = watched_files;
        self.watched_dirs = watched_dirs;
        Ok(())
    }
}

fn forward_event(
    event: notify::Event,
    watched_files: &Arc<Mutex<HashSet<PathBuf>>>,
    on_event: &mut impl FnMut(WatchEvent),
) {
    let matching_events = {
        let watched_files = watched_files.lock().unwrap();
        if watched_files.is_empty() {
            return;
        }

        classify_event(event)
            .into_iter()
            .filter(|(path, _)| watched_files.contains(path))
            .collect::<Vec<_>>()
    };

    for (path, kind) in matching_events {
        on_event(WatchEvent { path, kind });
    }
}

fn classify_event(event: notify::Event) -> Vec<(PathBuf, FileChangeKind)> {
    use notify::EventKind;
    use notify::event::{ModifyKind, RenameMode};

    fn map_event(event: notify::Event, kind: FileChangeKind) -> Vec<(PathBuf, FileChangeKind)> {
        event
            .paths
            .into_iter()
            .map(|path| (i_slint_compiler::pathutils::clean_path(&path), kind))
            .collect()
    }

    match event.kind {
        EventKind::Create(_) => map_event(event, FileChangeKind::Created),
        EventKind::Remove(_) => map_event(event, FileChangeKind::Deleted),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            map_event(event, FileChangeKind::Deleted)
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            map_event(event, FileChangeKind::Created)
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            let mut paths = event.paths.into_iter();
            [
                paths.next().map(|path| {
                    (i_slint_compiler::pathutils::clean_path(&path), FileChangeKind::Deleted)
                }),
                paths.next().map(|path| {
                    (i_slint_compiler::pathutils::clean_path(&path), FileChangeKind::Created)
                }),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        EventKind::Modify(_) => map_event(event, FileChangeKind::Changed),
        _ => Vec::new(),
    }
}

fn watch_directory_for_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    parent.is_dir().then(|| i_slint_compiler::pathutils::clean_path(parent))
}

fn watch_mode() -> notify::RecursiveMode {
    if cfg!(target_vendor = "apple") {
        notify::RecursiveMode::Recursive
    } else {
        notify::RecursiveMode::NonRecursive
    }
}
