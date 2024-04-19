// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

mod search_api;
pub use search_api::*;
#[cfg(feature = "internal")]
mod internal_tests;
#[cfg(feature = "internal")]
pub use internal_tests::*;
mod testing_backend;
#[cfg(feature = "internal")]
pub use testing_backend::*;
#[cfg(feature = "ffi")]
mod ffi;

/// Initialize the testing backend without support for event loop.
/// This means that each test thread can use its own backend, but global functions that needs
/// an event loop such as `slint::invoke_from_event_loop` or `Timer`s won't work.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will panic.
pub fn init_no_event_loop() {
    i_slint_core::platform::set_platform(
        Box::new(testing_backend::TestingBackend::new_no_thread()),
    )
    .expect("platform already initialized");
}

/// Initialize the testing backend with support for simple event loop.
/// This function can only be called once per process, so make sure to use integration
/// tests with only one `#[test]` function. (Or in a doc test)
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will panic.
pub fn init_integration_test() {
    i_slint_core::platform::set_platform(Box::new(testing_backend::TestingBackend::new()))
        .expect("platform already initialized");
}
