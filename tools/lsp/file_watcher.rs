// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use lsp_types::{FileChangeType, Url};
use notify::Watcher;

use crate::common::Result;

pub struct WatchEvent {
    pub url: Url,
    pub typ: FileChangeType,
}

pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    watched_dirs: HashSet<PathBuf>,
}

impl FileWatcher {
    pub fn start() -> Result<(Self, tokio::sync::mpsc::UnboundedReceiver<WatchEvent>)> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let watcher =
            notify::recommended_watcher(
                move |result: notify::Result<notify::Event>| match result {
                    Ok(event) => forward_event(event, &tx),
                    Err(err) => tracing::warn!("File watcher error: {err}"),
                },
            )?;

        Ok((Self { watcher, watched_dirs: HashSet::new() }, rx))
    }

    /// A list of paths to watch.
    /// The file-watcher will figure out the smallest number of directories
    /// to watch recursively to cover all the given paths.
    ///
    /// So the [`FileWatcher`] may produce additional events for files that are not in the given list.
    pub fn update_watched_paths<I>(&mut self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let new_watched_dirs = watch_dirs(paths);

        for dir in self.watched_dirs.difference(&new_watched_dirs) {
            self.watcher.unwatch(dir)?;
        }

        for dir in new_watched_dirs.difference(&self.watched_dirs) {
            self.watcher.watch(dir, notify::RecursiveMode::Recursive)?;
        }

        tracing::debug!("Watching directories recursively: {new_watched_dirs:?}");
        self.watched_dirs = new_watched_dirs;
        Ok(())
    }
}

fn forward_event(event: notify::Event, tx: &tokio::sync::mpsc::UnboundedSender<WatchEvent>) {
    use notify::EventKind;
    use notify::event::{ModifyKind, RenameMode};

    fn map_event(
        event: notify::Event,
        file_type: FileChangeType,
    ) -> Vec<(PathBuf, FileChangeType)> {
        event.paths.into_iter().map(|path| (path, file_type)).collect()
    }
    let changes = match event.kind {
        EventKind::Create(_) => map_event(event, FileChangeType::CREATED),
        EventKind::Remove(_) => map_event(event, FileChangeType::DELETED),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            map_event(event, FileChangeType::DELETED)
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            map_event(event, FileChangeType::CREATED)
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            let mut paths = event.paths.into_iter();
            [
                paths.next().map(|path| (path, FileChangeType::DELETED)),
                paths.next().map(|path| (path, FileChangeType::CREATED)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        }
        EventKind::Modify(_) => map_event(event, FileChangeType::CHANGED),
        _ => return,
    };

    for (path, typ) in changes {
        let path = i_slint_compiler::pathutils::clean_path(&path);
        let Ok(url) = Url::from_file_path(&path) else {
            tracing::debug!("Ignoring file watcher event for non-file path: {}", path.display());
            continue;
        };

        if tx.send(WatchEvent { url, typ }).is_err() {
            break;
        }
    }
}

fn watch_directory_for_path(path: PathBuf) -> Option<PathBuf> {
    let mut current = path.parent()?.to_path_buf();

    while !current.is_dir() {
        current = current.parent()?.to_path_buf();
    }

    if current.parent().is_none() {
        // TODO: If we receive an errouneous file path, we have to watch the highest directory in
        // the filesystem tree that does actually exist.
        // If the path is entirely non-existent, this will default to the file-system root.
        // We can avoid this worst-case with the check here, but one could still end up watching all of /usr if
        // we are told to watch a non-existent path like /usr/nonexistent/file.slint.
        //
        // To fully avoid this, we would need to use non-recursive directory watching, which
        // requires a lot of a extra tracking.
        // In practice, the current approach should be good enough, as paths are usually relative,
        // which means they are bound to the project directory anyway.
        tracing::debug!(
            "Skipping file watcher for {} because it would require watching the filesystem root",
            path.display()
        );
        return None;
    }

    Some(i_slint_compiler::pathutils::clean_path(Path::new(&current)))
}

/// Figure out the smallest number of directories we need to watch recursively
/// to cover all the given paths, and avoid watching parent directories of already watched directories.
fn watch_dirs<I>(paths: I) -> HashSet<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut dirs = paths.into_iter().filter_map(watch_directory_for_path).collect::<Vec<_>>();
    dirs.sort_by_key(|dir| dir.components().count());

    let mut result = Vec::new();
    for dir in dirs {
        if result.iter().any(|existing| dir.starts_with(existing)) {
            continue;
        }

        result.push(dir);
    }

    result.into_iter().collect()
}
