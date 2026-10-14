// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Standalone harness: replace Slint's single-threaded storage with standard
// thread-local storage, while testing the production callback and frame clock.
extern crate alloc;
extern crate self as i_slint_core;
pub use std::thread_local;
#[path = "../animation_frame.rs"]
mod animation_frame;
