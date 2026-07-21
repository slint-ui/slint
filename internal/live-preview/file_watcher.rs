// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore inotify unwatch
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

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

/// A file-system event for one path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    /// The affected path.
    ///
    /// Most file watchers only emit watched paths.
    /// Optionally a FileWatcher can be created with pass_through_unwatched_events = true, in which
    /// case the path may not be on the list of watched paths.
    pub path: PathBuf,
    /// The normalized change kind for this path.
    pub kind: FileChangeKind,
}

/// The underlying file system watcher implementation (based on notify::RecommendedWatcher by
/// default)
pub trait FileWatcherImpl: Sized + 'static {
    /// The error type used by the [`FileWatcher`].
    type Error: Send;

    /// Add the given path to the watch set.
    ///
    /// Note that the reconcile loop expects to receive events for this path after a successful
    /// call, so implementations should not return until the watch is fully active and events
    /// will be received.
    fn watch(&mut self, path: &Path) -> Result<(), Self::Error>;

    /// Remove the given path from the watch set.
    fn unwatch(&mut self, path: &Path) -> Result<(), Self::Error>;

    /// The error to return when the worker thread has stopped and can no longer process messages.
    fn worker_stopped_error() -> Self::Error;

    /// Whether the error is transient and should not be reported to the user, for example
    /// because it may occur during normal operation when files or probe directories are removed.
    fn is_transient_watch_error(err: &Self::Error) -> bool;

    /// Whether the implementation needs to watch target files directly in order to receive change
    /// events, or if watching parent directories is sufficient.
    fn needs_direct_file_watches() -> bool;
}

impl FileWatcherImpl for notify::RecommendedWatcher {
    type Error = notify::Error;

    fn worker_stopped_error() -> Self::Error {
        notify::Error::generic("file watcher worker thread stopped")
    }

    fn is_transient_watch_error(err: &Self::Error) -> bool {
        match &err.kind {
            notify::ErrorKind::PathNotFound
            | notify::ErrorKind::WatchNotFound
            | notify::ErrorKind::Generic(_) => true,
            notify::ErrorKind::Io(e) => e.kind() == std::io::ErrorKind::NotFound,
            _ => false,
        }
    }

    fn watch(&mut self, path: &Path) -> Result<(), Self::Error> {
        notify::Watcher::watch(self, path, notify::RecursiveMode::NonRecursive)
    }

    fn unwatch(&mut self, path: &Path) -> Result<(), Self::Error> {
        notify::Watcher::unwatch(self, path)
    }

    fn needs_direct_file_watches() -> bool {
        // On macOS, notify does not report file changed events, if we only watch the parent
        // directory, so we need to add a direct file watch as well.
        cfg!(target_os = "macos")
    }
}

/// A file watcher for a set of source or resource paths.
///
/// Given a set of existing and/or non-existing paths, the file watcher will try to minimize the
/// number of actual OS file watches needed to receive events for all the given paths.
/// It reconciles the events with the file state and disk and synthesizes  file change events if
/// e.g. a file in a new directory is created before the OS watch could be set up.
///
/// Communication with the OS is abstracted behind a low-level trait ([`FileWatcherImpl`]) which is responsible for
/// delivering the original file watcher events.
/// This allows the file watcher to be used with the OS APIs (i.e. notify) or the LSP file watcher.
pub struct FileWatcher<Impl: FileWatcherImpl = notify::RecommendedWatcher> {
    tx: mpsc::Sender<WorkerMessage<Impl>>,

    /// Use a worker thread for processing file events and updating watches.
    ///
    /// `notify` already invokes callbacks from backend-managed threads/event loops, but
    /// reconcile performs `watch()` / `unwatch()` calls as it updates probe directories.
    /// Backends such as inotify and kqueue route those operations through the same backend
    /// loop and wait synchronously for an acknowledgement, so running reconcile directly in
    /// the callback can deadlock. The dedicated worker thread keeps that work off the
    /// backend callback thread while still serializing all watcher state transitions.
    worker: Option<JoinHandle<()>>,
}

impl FileWatcher<notify::RecommendedWatcher> {
    /// Creates a watcher based on the notify crate and invokes `on_event` for matching watched-path changes.
    ///
    /// Runtime watcher errors are forwarded to `on_error`.
    pub fn start(
        on_event: impl FnMut(WatchEvent) + Send + 'static,
        on_error: impl FnMut(notify::Error) + Send + 'static,
    ) -> Result<Self, notify::Error> {
        Self::start_with_impl(on_event, on_error, false, |event_handler| {
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                // Keep the backend callback lightweight and forward the real work to the worker.
                //
                // This is especially needed on inotify backends, where calling watch/unwatch within
                // the callback can cause a deadlock.
                let events = event.map(classify_event);
                event_handler.send(events).ok();
            })
        })
    }
}

impl<Impl: FileWatcherImpl> FileWatcher<Impl> {
    /// Creates a watcher and invokes `on_event` for matching watched-path changes.
    ///
    /// If `pass_through_unwatched_events` is `true`, raw file-system events for paths outside the
    /// current watched set are forwarded as-is.
    ///
    /// Runtime watcher errors are forwarded to `on_error`.
    pub fn start_with_impl(
        on_event: impl FnMut(WatchEvent) + Send + 'static,
        on_error: impl FnMut(Impl::Error) + Send + 'static,
        pass_through_unwatched_events: bool,
        create_impl: impl FnOnce(FileWatcherEventSink<Impl>) -> Result<Impl, Impl::Error>
        + Send
        + 'static,
    ) -> Result<Self, Impl::Error> {
        let (tx, rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let worker_tx = tx.clone();
        let worker = thread::spawn(move || {
            worker_loop(
                create_impl,
                rx,
                worker_tx,
                startup_tx,
                on_event,
                on_error,
                pass_through_unwatched_events,
            );
        });

        match startup_rx.recv() {
            Ok(Ok(())) => Ok(Self { tx, worker: Some(worker) }),
            Ok(Err(err)) => {
                let _ = worker.join();
                Err(err)
            }
            Err(_) => {
                let _ = worker.join();
                Err(Impl::worker_stopped_error())
            }
        }
    }

    /// Replaces the watched path set with `paths`.
    ///
    /// Paths must be absolute: they are compared by value against the paths the backend reports
    /// for events, which are absolute, and relative paths are never resolved against a base.
    pub fn update_watched_paths<I>(&mut self, paths: I) -> Result<(), Impl::Error>
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let watched_files = paths
            .into_iter()
            .map(|path| i_slint_compiler::pathutils::clean_path(&path))
            .collect::<HashSet<_>>();

        let (response_tx, response_rx) = mpsc::sync_channel(1);
        self.tx
            .send(WorkerMessage::UpdateWatchedPaths { watched_files, response: response_tx })
            .map_err(|_| Impl::worker_stopped_error())?;
        response_rx.recv().map_err(|_| Impl::worker_stopped_error())?
    }

    /// The [`FileWatcherImpl`] receives a [`FileWatcherEventSink`] on creation.
    /// However, it is created on the reconcile worker thread, so the caller may not have access to
    /// it anymore.
    ///
    /// This method can be used to get a new sink at any time, for example to send events
    /// from the main thread.
    pub fn event_sink(&mut self) -> FileWatcherEventSink<Impl> {
        FileWatcherEventSink { tx: self.tx.clone() }
    }
}

impl<Impl: FileWatcherImpl> Drop for FileWatcher<Impl> {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
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

enum WorkerMessage<Impl: FileWatcherImpl> {
    UpdateWatchedPaths {
        watched_files: HashSet<PathBuf>,
        response: mpsc::SyncSender<Result<(), Impl::Error>>,
    },
    FileSystemEvents(Result<Vec<(PathBuf, FileChangeKind)>, Impl::Error>),
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetState {
    Existing { probe_dir: Option<PathBuf> },
    Missing { probe_dir: Option<PathBuf> },
}

impl TargetState {
    fn exists(&self) -> bool {
        matches!(self, Self::Existing { .. })
    }

    fn probe_dir(&self) -> Option<&PathBuf> {
        match self {
            Self::Existing { probe_dir } | Self::Missing { probe_dir } => probe_dir.as_ref(),
        }
    }
}

#[derive(Default, Debug)]
struct WorkerState {
    /// The set of paths to watch
    watched_files: HashSet<PathBuf>,
    target_states: HashMap<PathBuf, TargetState>,
    /// The set of actually registered watch paths, which may include probe directories and/or directly watched files.
    registered_watches: HashSet<PathBuf>,
}

impl WorkerState {
    fn update_watched_paths<Impl: FileWatcherImpl>(
        &mut self,
        watcher: &mut Impl,
        watched_files: HashSet<PathBuf>,
        on_event: &mut impl FnMut(WatchEvent),
    ) -> Result<(), Impl::Error> {
        let previous_states = watched_files
            .iter()
            .map(|path| {
                let state = self
                    .target_states
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| scan_target_state(path));
                (path.clone(), state)
            })
            .collect::<HashMap<_, _>>();

        self.watched_files = watched_files;
        self.target_states = previous_states.clone();
        self.reconcile(watcher, previous_states, HashSet::new(), on_event)
    }

    fn handle_events<Impl: FileWatcherImpl>(
        &mut self,
        watcher: &mut Impl,
        events: Vec<(PathBuf, FileChangeKind)>,
        pass_through_unwatched_events: bool,
        on_event: &mut impl FnMut(WatchEvent),
    ) -> Result<(), Impl::Error> {
        if pass_through_unwatched_events {
            let passthrough_events = events
                .iter()
                .filter(|(path, _kind)| !self.watched_files.contains(path))
                .map(|(path, kind)| WatchEvent { path: path.clone(), kind: *kind });
            for event in passthrough_events {
                on_event(event);
            }
        }

        if self.watched_files.is_empty() {
            return Ok(());
        }

        let previous_states = self.target_states.clone();
        let changed_paths = events
            .into_iter()
            .filter_map(|(path, kind)| {
                (kind == FileChangeKind::Changed && self.watched_files.contains(&path))
                    .then_some(path)
            })
            .collect::<HashSet<_>>();

        self.reconcile(watcher, previous_states, changed_paths, on_event)
    }

    fn reconcile<Impl: FileWatcherImpl>(
        &mut self,
        watcher: &mut Impl,
        previous_states: HashMap<PathBuf, TargetState>,
        changed_paths: HashSet<PathBuf>,
        on_event: &mut impl FnMut(WatchEvent),
    ) -> Result<(), Impl::Error> {
        const MAX_RECONCILE_PASSES: usize = 8;

        let mut target_states = scan_target_states(&self.watched_files);

        for _ in 0..MAX_RECONCILE_PASSES {
            let desired_watches = desired_watches_for_states::<Impl>(&target_states);
            if desired_watches == self.registered_watches {
                break;
            }

            self.apply_watch_plan(watcher, &desired_watches)?;
            target_states = scan_target_states(&self.watched_files);
        }

        self.target_states = target_states;

        let mut transitioned_paths = HashSet::new();
        for path in &self.watched_files {
            let previous = previous_states.get(path).map(TargetState::exists).unwrap_or(false);
            let current = self.target_states.get(path).map(TargetState::exists).unwrap_or(false);

            match (previous, current) {
                (false, true) => {
                    transitioned_paths.insert(path.clone());
                    on_event(WatchEvent { path: path.clone(), kind: FileChangeKind::Created });
                }
                (true, false) => {
                    transitioned_paths.insert(path.clone());
                    on_event(WatchEvent { path: path.clone(), kind: FileChangeKind::Deleted });
                }
                _ => {}
            }
        }

        for path in changed_paths {
            if transitioned_paths.contains(&path) {
                continue;
            }

            if self.target_states.get(&path).map(TargetState::exists).unwrap_or(false) {
                on_event(WatchEvent { path, kind: FileChangeKind::Changed });
            }
        }

        Ok(())
    }

    fn apply_watch_plan<Impl: FileWatcherImpl>(
        &mut self,
        watcher: &mut Impl,
        desired_registrations: &HashSet<PathBuf>,
    ) -> Result<(), Impl::Error> {
        let current_watches = self.registered_watches.clone();

        for registration in desired_registrations.difference(&current_watches) {
            match watcher.watch(registration) {
                Ok(()) => {
                    self.registered_watches.insert(registration.clone());
                }
                Err(err) if Impl::is_transient_watch_error(&err) => {}
                Err(err) => return Err(err),
            }
        }

        for registration in current_watches.difference(desired_registrations) {
            match watcher.unwatch(registration) {
                Ok(()) => {}
                Err(err) if Impl::is_transient_watch_error(&err) => {}
                Err(err) => return Err(err),
            }
            self.registered_watches.remove(registration);
        }

        Ok(())
    }
}

/// A handle to the file watcher worker, allowing it to receive file system events and
/// update watches from the [`FileWatcherImpl`].
pub struct FileWatcherEventSink<Impl: FileWatcherImpl> {
    tx: mpsc::Sender<WorkerMessage<Impl>>,
}

impl<Impl: FileWatcherImpl> FileWatcherEventSink<Impl> {
    /// Send a file system event to the worker from the [`FileWatcherImpl`].
    pub fn send(
        &self,
        events: Result<Vec<(PathBuf, FileChangeKind)>, Impl::Error>,
    ) -> Result<(), Impl::Error> {
        self.tx
            .send(WorkerMessage::FileSystemEvents(events))
            .map_err(|_| Impl::worker_stopped_error())
    }
}

fn worker_loop<Impl: FileWatcherImpl>(
    create_impl: impl FnOnce(FileWatcherEventSink<Impl>) -> Result<Impl, Impl::Error> + Send + 'static,
    rx: mpsc::Receiver<WorkerMessage<Impl>>,
    tx: mpsc::Sender<WorkerMessage<Impl>>,
    startup_tx: mpsc::SyncSender<Result<(), Impl::Error>>,
    mut on_event: impl FnMut(WatchEvent) + Send + 'static,
    mut on_error: impl FnMut(Impl::Error) + Send + 'static,
    pass_through_unwatched_events: bool,
) {
    let watcher = create_impl(FileWatcherEventSink { tx });

    let mut watcher = match watcher {
        Ok(watcher) => {
            let _ = startup_tx.send(Ok(()));
            watcher
        }
        Err(err) => {
            let _ = startup_tx.send(Err(err));
            return;
        }
    };

    let mut state = WorkerState::default();

    while let Ok(message) = rx.recv() {
        match message {
            WorkerMessage::UpdateWatchedPaths { watched_files, response } => {
                let _ = response.send(state.update_watched_paths(
                    &mut watcher,
                    watched_files,
                    &mut on_event,
                ));
            }
            WorkerMessage::FileSystemEvents(Ok(event)) => {
                if let Err(err) = state.handle_events(
                    &mut watcher,
                    event,
                    pass_through_unwatched_events,
                    &mut on_event,
                ) {
                    on_error(err);
                }
            }
            WorkerMessage::FileSystemEvents(Err(err)) => {
                if !Impl::is_transient_watch_error(&err) {
                    on_error(err);
                }
            }
            WorkerMessage::Shutdown => break,
        }
    }
}

fn scan_target_states(watched_files: &HashSet<PathBuf>) -> HashMap<PathBuf, TargetState> {
    watched_files.iter().map(|path| (path.clone(), scan_target_state(path))).collect()
}

fn scan_target_state(path: &Path) -> TargetState {
    let probe_dir = probe_dir_for_path(path);
    if path.exists() {
        TargetState::Existing { probe_dir }
    } else {
        TargetState::Missing { probe_dir }
    }
}

fn desired_watches_for_states<Impl: FileWatcherImpl>(
    target_states: &HashMap<PathBuf, TargetState>,
) -> HashSet<PathBuf> {
    let mut watches = target_states
        .values()
        .filter_map(|state| state.probe_dir().cloned())
        .collect::<HashSet<_>>();

    if Impl::needs_direct_file_watches() {
        watches.extend(
            target_states
                .iter()
                .filter(|(_path, state)| state.exists())
                .map(|(path, _state)| path.clone()),
        );
    }

    watches
}

fn probe_dir_for_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        let parent = path.parent()?;
        parent.is_dir().then(|| i_slint_compiler::pathutils::clean_path(parent))
    } else {
        nearest_existing_ancestor(path)
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    while !current.is_dir() {
        current = current.parent()?;
    }

    Some(i_slint_compiler::pathutils::clean_path(current))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const WATCHER_SETTLE_DELAY: Duration = Duration::from_millis(50);
    const EVENT_TIMEOUT: Duration = Duration::from_secs(1);
    const QUIET_TIMEOUT: Duration = Duration::from_millis(50);

    fn new_test_root() -> PathBuf {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir()
            .join(format!("slint-file-watcher-{timestamp}-{unique_id}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    struct TestContext {
        root: PathBuf,
        watcher: FileWatcher,
        events: Receiver<WatchEvent>,
        errors: Receiver<notify::Error>,
    }

    impl TestContext {
        fn new() -> Self {
            Self::new_with_passthrough(false)
        }

        fn new_with_passthrough(pass_through_unwatched_events: bool) -> Self {
            let root = new_test_root();
            let (event_tx, events) = mpsc::channel();
            let (error_tx, errors) = mpsc::channel();

            let watcher = FileWatcher::start_with_impl(
                move |event| {
                    event_tx.send(event).unwrap();
                },
                move |error| {
                    error_tx.send(error).unwrap();
                },
                pass_through_unwatched_events,
                |event_handler| {
                    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                        let events = event.map(classify_event);
                        event_handler.send(events).ok();
                    })
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

        fn remove_dir_all(&self, relative: impl AsRef<Path>) {
            fs::remove_dir_all(self.path(relative)).unwrap();
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

        fn send_raw(&mut self, events: Vec<(PathBuf, FileChangeKind)>) {
            self.watcher.event_sink().send(Ok(events)).unwrap();
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
    fn passthrough_can_emit_unwatched_paths() {
        let mut ctx = TestContext::new_with_passthrough(true);
        let unwatched = ctx.path("ui/unwatched.slint");

        ctx.send_raw(vec![(unwatched.clone(), FileChangeKind::Changed)]);

        ctx.expect_event(&unwatched, FileChangeKind::Changed);
        ctx.expect_quiet();
    }

    #[test]
    fn passthrough_works_without_watched_paths() {
        let mut ctx = TestContext::new_with_passthrough(true);
        let unwatched = ctx.path("ui/unwatched.slint");

        ctx.send_raw(vec![(unwatched.clone(), FileChangeKind::Created)]);

        ctx.expect_event(&unwatched, FileChangeKind::Created);
        ctx.expect_quiet();
    }

    #[test]
    fn passthrough_does_not_leak_without_opt_in() {
        let mut ctx = TestContext::new();
        let unwatched = ctx.path("ui/unwatched.slint");

        ctx.send_raw(vec![(unwatched, FileChangeKind::Changed)]);

        ctx.expect_quiet();
    }

    #[test]
    fn passthrough_still_reconciles_watched_paths() {
        let mut ctx = TestContext::new_with_passthrough(true);
        let watched = ctx.path("ui/missing.slint");

        ctx.create_dir_all("ui");
        let sibling = ctx.write("ui/sibling.slint", "first");
        ctx.watch(&["ui/missing.slint"]);
        ctx.write("ui/missing.slint", "created later");
        ctx.expect_event(&watched, FileChangeKind::Created);

        ctx.send_raw(vec![(sibling.clone(), FileChangeKind::Changed)]);
        ctx.expect_event(&sibling, FileChangeKind::Changed);
        ctx.settle();

        for event in ctx.drain_events() {
            assert_eq!(
                event,
                WatchEvent { path: watched.clone(), kind: FileChangeKind::Changed },
                "unexpected trailing event after reconcile",
            );
        }
    }

    #[test]
    fn passthrough_does_not_duplicate_watched_events() {
        let mut ctx = TestContext::new_with_passthrough(true);
        let watched = ctx.write("ui/main.slint", "first");

        ctx.watch(&["ui/main.slint"]);
        ctx.remove_file("ui/main.slint");
        ctx.send_raw(vec![(watched.clone(), FileChangeKind::Deleted)]);

        ctx.expect_event(&watched, FileChangeKind::Deleted);
        let folder_path = ctx.path("ui");

        ctx.settle();
        // On some platforms, removing a file creates a file change event on the folder (e.g. on macOS)
        // Only allow this, but no other event to occur
        for event in ctx.drain_events() {
            assert_eq!(
                event,
                WatchEvent { path: folder_path.clone(), kind: FileChangeKind::Changed },
            )
        }
        ctx.expect_quiet();
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

    #[test]
    fn refreshing_after_probe_directory_is_removed_recovers_cleanly() {
        let mut ctx = TestContext::new();
        ctx.write("test.slint", "export component Test { }");
        let watched_nested = ctx.write("thing/thing.slint", "export component Thing { }");

        ctx.watch(&["test.slint", "thing/thing.slint"]);
        ctx.remove_dir_all("thing");
        ctx.settle();
        ctx.expect_event(&watched_nested, FileChangeKind::Deleted);
        ctx.drain_events();
        ctx.assert_no_errors();

        ctx.watch(&["test.slint", "thing/thing.slint"]);

        ctx.write("thing/thing.slint", "export component Thing { in property<string> x; }");
        ctx.expect_event(&watched_nested, FileChangeKind::Created);
    }

    #[test]
    fn removing_watched_directory_does_not_report_spurious_errors() {
        let mut ctx = TestContext::new();
        let watched = ctx.write("project/src/main.slint", "export component App { }");

        ctx.watch(&["project/src/main.slint"]);
        ctx.remove_dir_all("project");
        ctx.expect_event(&watched, FileChangeKind::Deleted);
        ctx.settle();
        ctx.assert_no_errors();
    }
}
