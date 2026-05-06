// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared scaffolding for integration tests that drive a `MinimalSoftwareWindow`.
//!
//! Each test file that uses this declares `mod common;` and calls
//! `common::setup(width, height)` to obtain the platform-installed window.

#![allow(dead_code)]

use slint::PhysicalSize;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{PlatformError, WindowAdapter};
use std::rc::Rc;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
}

struct TestPlatform;

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|x| x.clone()))
    }
}

/// Install `TestPlatform` (idempotent across tests in the same binary) and resize
/// the shared window to `width` x `height` physical pixels.
pub fn setup(width: u32, height: u32) -> Rc<MinimalSoftwareWindow> {
    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(PhysicalSize::new(width, height));
    window
}
