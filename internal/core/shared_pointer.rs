// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The reference-counted pointer for data that a renderer may share with another thread.
//!
//! [`SharedPtr`] is `Arc` where the target has atomic pointers, so that a type holding it can be
//! `Send` and `Sync`.
//! Targets without them, such as `thumbv6m` where `alloc::sync` doesn't exist, get `Rc`:
//! its counts need no critical section, and a type holding it stays `!Send` and `!Sync`.
//! Write a `Send` or `Sync` assertion for such a type under `cfg(target_has_atomic = "ptr")`.

#[cfg(target_has_atomic = "ptr")]
pub use alloc::sync::{Arc as SharedPtr, Weak as SharedWeak};

#[cfg(not(target_has_atomic = "ptr"))]
pub use alloc::rc::{Rc as SharedPtr, Weak as SharedWeak};
