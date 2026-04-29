// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::Watcher as _;

/// A normalized file-system change emitted by [`FileWatcher`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileChangeKind {
    /// A watched file appeared on disk.
    Created,
    /// A watched file changed on disk.
    Changed,
    /// A watched file disappeared from disk.
    Deleted,
}

/// A file-system event for one watched path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    /// The affected watched path.
    pub path: PathBuf,
    /// The normalized change kind for this path.
    pub kind: FileChangeKind,
}

/// A native file watcher for a set of source or resource paths.
pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    watched_files: Arc<Mutex<HashSet<PathBuf>>>,
    watched_dirs: HashSet<PathBuf>,
}

impl FileWatcher {
    /// Creates a watcher and invokes `on_event` for matching watched-path changes.
    ///
    /// Runtime watcher errors are forwarded to `on_error`.
    pub fn start(
        mut on_event: impl FnMut(WatchEvent) + Send + 'static,
        mut on_error: impl FnMut(notify::Error) + Send + 'static,
    ) -> notify::Result<Self> {
        let watched_files = Arc::new(Mutex::new(HashSet::new()));
        let callback_files = watched_files.clone();
        let watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) => forward_event(event, &callback_files, &mut on_event),
                Err(err) => on_error(err),
            })?;

        Ok(Self { watcher, watched_files, watched_dirs: HashSet::new() })
    }

    /// Replaces the watched path set with `paths`.
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const WATCHER_SETTLE_DELAY: Duration = Duration::from_millis(50);
    const EVENT_TIMEOUT: Duration = Duration::from_millis(100);
    const QUIET_TIMEOUT: Duration = Duration::from_millis(50);

    struct TestContext {
        root: PathBuf,
        watcher: FileWatcher,
        events: Receiver<WatchEvent>,
        errors: Receiver<notify::Error>,
    }

    impl TestContext {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

            let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = std::env::temp_dir()
                .join(format!("slint-file-watcher-{timestamp}-{unique_id}-{}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            let (event_tx, events) = mpsc::channel();
            let (error_tx, errors) = mpsc::channel();

            let watcher = FileWatcher::start(
                move |event| {
                    event_tx.send(event).unwrap();
                },
                move |error| {
                    error_tx.send(error).unwrap();
                },
            )
            .unwrap();

            Self { root, watcher, events, errors }
        }

        fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
            self.root.join(relative)
        }

        fn create_dir_all(&self, relative: impl AsRef<Path>) -> PathBuf {
            let path = self.path(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn write(&self, relative: impl AsRef<Path>, contents: &str) -> PathBuf {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, contents).unwrap();
            path
        }

        fn remove_file(&self, relative: impl AsRef<Path>) {
            fs::remove_file(self.path(relative)).unwrap();
        }

        fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) {
            let from = self.path(from);
            let to = self.path(to);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::rename(from, to).unwrap();
        }

        fn watch(&mut self, relative_paths: &[&str]) {
            let paths = relative_paths.iter().map(|path| self.path(*path)).collect::<Vec<_>>();
            self.watcher.update_watched_paths(paths).unwrap();
            self.settle();
            self.drain_events();
            self.assert_no_errors();
        }

        fn settle(&self) {
            std::thread::sleep(WATCHER_SETTLE_DELAY);
        }

        fn drain_events(&self) -> Vec<WatchEvent> {
            let mut events = Vec::new();
            while let Ok(event) = self.events.try_recv() {
                events.push(event);
            }
            events
        }

        fn drain_errors(&self) -> Vec<notify::Error> {
            let mut errors = Vec::new();
            while let Ok(error) = self.errors.try_recv() {
                errors.push(error);
            }
            errors
        }

        fn assert_no_errors(&self) {
            let errors = self.drain_errors();
            assert!(errors.is_empty(), "unexpected watcher errors: {errors:?}");
        }

        fn expect_event(&self, path: &Path, kind: FileChangeKind) {
            let expected = WatchEvent { path: path.to_path_buf(), kind };
            let deadline = Instant::now() + EVENT_TIMEOUT;
            let mut seen = Vec::new();

            loop {
                self.assert_no_errors();

                match self.events.recv_timeout(EVENT_TIMEOUT) {
                    Ok(event) if event == expected => return,
                    Ok(event) => seen.push(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        panic!("timed out waiting for {expected:?}; saw {seen:?}")
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        panic!("watcher event channel disconnected while waiting for {expected:?}")
                    }
                }
            }
        }

        fn expect_quiet(&self) {
            let deadline = Instant::now() + QUIET_TIMEOUT;

            match self.events.recv_timeout(QUIET_TIMEOUT) {
                Ok(event) => panic!("unexpected event during quiet period: {event:?}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("watcher event channel disconnected while waiting for quiet period")
                }
            }

            self.assert_no_errors();
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn reports_changed_for_existing_watched_file() {
        let mut ctx = TestContext::new();
        let watched = ctx.write("ui/main.slint", "first");

        ctx.watch(&["ui/main.slint"]);
        ctx.write("ui/main.slint", "second");

        ctx.expect_event(&watched, FileChangeKind::Changed);
    }

    #[test]
    fn reports_deleted_and_created_for_existing_watched_file() {
        let mut ctx = TestContext::new();
        let watched = ctx.write("ui/main.slint", "first");

        ctx.watch(&["ui/main.slint"]);
        ctx.remove_file("ui/main.slint");
        ctx.expect_event(&watched, FileChangeKind::Deleted);

        ctx.write("ui/main.slint", "second");
        ctx.expect_event(&watched, FileChangeKind::Created);
    }

    #[test]
    fn reports_deleted_when_watched_file_is_renamed_away() {
        let mut ctx = TestContext::new();
        let watched = ctx.write("ui/main.slint", "first");

        ctx.watch(&["ui/main.slint"]);
        ctx.rename("ui/main.slint", "ui/renamed.slint");

        ctx.expect_event(&watched, FileChangeKind::Deleted);
    }

    #[test]
    fn reports_created_when_file_is_renamed_into_watched_path() {
        let mut ctx = TestContext::new();
        let watched = ctx.path("ui/main.slint");

        ctx.create_dir_all("ui");
        ctx.write("ui/temp.slint", "temporary");
        ctx.watch(&["ui/main.slint"]);
        ctx.drain_events();

        ctx.rename("ui/temp.slint", "ui/main.slint");

        ctx.expect_event(&watched, FileChangeKind::Created);
    }

    #[test]
    fn ignores_changes_to_unwatched_sibling_files() {
        let mut ctx = TestContext::new();
        ctx.write("ui/main.slint", "main");
        ctx.write("ui/sibling.slint", "sibling");

        ctx.watch(&["ui/main.slint"]);
        ctx.write("ui/sibling.slint", "sibling changed");

        ctx.expect_quiet();
    }

    #[test]
    fn reports_created_for_missing_file_when_parent_directory_exists() {
        let mut ctx = TestContext::new();
        let watched = ctx.path("ui/missing.slint");

        ctx.create_dir_all("ui");
        ctx.watch(&["ui/missing.slint"]);
        ctx.write("ui/missing.slint", "created later");

        ctx.expect_event(&watched, FileChangeKind::Created);
    }

    #[test]
    fn reports_created_for_missing_file_when_intermediate_directory_is_created_later() {
        let mut ctx = TestContext::new();
        let watched = ctx.path("ui/generated/missing.slint");

        ctx.create_dir_all("ui");
        ctx.watch(&["ui/generated/missing.slint"]);
        ctx.write("ui/generated/missing.slint", "created with parent later");

        ctx.expect_event(&watched, FileChangeKind::Created);
    }

    #[test]
    fn reports_created_for_missing_file_when_directory_chain_is_created_later() {
        let mut ctx = TestContext::new();
        let watched = ctx.path("ui/generated/deep/missing.slint");

        ctx.watch(&["ui/generated/deep/missing.slint"]);
        ctx.write("ui/generated/deep/missing.slint", "created with full chain later");

        ctx.expect_event(&watched, FileChangeKind::Created);
    }

    #[test]
    fn refreshing_watch_set_stops_forwarding_old_paths() {
        let mut ctx = TestContext::new();
        let first = ctx.write("ui/first.slint", "first");
        let second = ctx.write("ui/second.slint", "first");

        ctx.watch(&["ui/first.slint"]);
        ctx.write("ui/first.slint", "first updated");
        ctx.expect_event(&first, FileChangeKind::Changed);
        ctx.drain_events();

        ctx.watch(&["ui/second.slint"]);
        ctx.write("ui/first.slint", "should now be ignored");
        ctx.expect_quiet();

        ctx.write("ui/second.slint", "second updated");
        ctx.expect_event(&second, FileChangeKind::Changed);
    }
}
