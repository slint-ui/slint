// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared scaffolding for integration tests that drive a `MinimalSoftwareWindow`.
//!
//! Each test file that uses this declares `mod common;` and calls
//! `common::setup(width, height)` to obtain the platform-installed window. Tests that need a
//! platform of their own -- one with a controllable clock, say -- call
//! [`setup_with_platform`] instead and hand out [`window`] from their `create_window_adapter`.

#![allow(dead_code)]

use slint::PhysicalSize;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use std::rc::Rc;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
}

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(window())
    }
}

/// The window shared by every test in a binary. Custom platforms must hand out this one.
pub fn window() -> Rc<MinimalSoftwareWindow> {
    WINDOW.with(|x| x.clone())
}

/// Install `TestPlatform` (idempotent across tests in the same binary) and resize
/// the shared window to `width` x `height` physical pixels.
pub fn setup(width: u32, height: u32) -> Rc<MinimalSoftwareWindow> {
    setup_with_platform(Box::new(TestPlatform), width, height)
}

/// Like [`setup`], but installs `platform` instead of the default one. Installing a platform only
/// takes effect once per process, so a test needing its own belongs in its own test binary.
pub fn setup_with_platform(
    platform: Box<dyn Platform>,
    width: u32,
    height: u32,
) -> Rc<MinimalSoftwareWindow> {
    slint::platform::set_platform(platform).ok();
    let window = window();
    window.set_size(PhysicalSize::new(width, height));
    window
}

/// A pixel that only records whether anything was drawn over it. Tests that assert on rendering
/// side effects rather than on colors render into a buffer of these.
#[derive(Clone, Copy, Default)]
pub struct TestPixel(pub bool);

impl TargetPixel for TestPixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }

    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}
