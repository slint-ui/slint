// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared setup for the live-reload tests. Each of them needs a test binary of
//! its own: the event loop runs once per process and the backend initializes
//! once, so they cannot share one.

// Not every test binary this is compiled into uses all of it.
#![allow(dead_code)]

use i_slint_live_preview::live_component::{Compiler, LiveReloadingComponent};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// Fail the assertions instead of hanging if an expected reload never arrives.
pub const RELOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// A directory of .slint files, removed when the test ends.
pub struct TestDir(tempfile::TempDir);

impl TestDir {
    pub fn new(name: &str) -> Self {
        Self(
            tempfile::Builder::new()
                .prefix(&format!("slint-live-reload-{name}-"))
                .tempdir()
                .unwrap(),
        )
    }

    /// The directory itself, for a test that writes to it after the setup.
    pub fn path(&self) -> &Path {
        self.0.path()
    }

    /// Write `contents` to `name` in this directory and return its path.
    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }
}

/// Start a component whose reloads compile on the worker thread, with a default
/// compiler. Initializes the backend, which every test binary has to do once.
pub fn start(file: &Path) -> Rc<RefCell<LiveReloadingComponent>> {
    start_with(Compiler::default, file)
}

/// [`start`] for a test that needs the worker's compiler configured.
pub fn start_with(
    factory: impl Fn() -> Compiler + Send + 'static,
    file: &Path,
) -> Rc<RefCell<LiveReloadingComponent>> {
    i_slint_backend_testing::init_integration_test_with_system_time();
    LiveReloadingComponent::new(factory, file.to_path_buf(), None).unwrap()
}

/// Quit the event loop after [`RELOAD_TIMEOUT`] whatever happened, so a test
/// that never reloads fails on its assertions instead of hanging.
pub fn watchdog() {
    after(RELOAD_TIMEOUT, || i_slint_core::api::quit_event_loop().unwrap());
}

/// Run `f` on the event loop after `delay`.
pub fn after(delay: Duration, f: impl FnOnce() + 'static) {
    i_slint_core::timers::Timer::single_shot(delay, f);
}
