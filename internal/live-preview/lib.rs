// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![cfg_attr(not(any(feature = "protocol", feature = "file-watcher")), no_std)]

extern crate alloc;

/// Window over which a burst of keystrokes from the editor is coalesced into a single
/// preview rebuild. Used by both the in-process LSP preview and the remote viewer so a
/// single value governs how reactive the preview feels.
pub const REBUILD_DEBOUNCE: core::time::Duration = core::time::Duration::from_millis(50);

/// Full Slint version of this build, shown in the discovery picker and in
/// version-mismatch error messages.
pub const SLINT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "protocol")]
pub mod protocol;

#[cfg(feature = "file-watcher")]
pub mod file_watcher;

#[cfg(feature = "live-component")]
pub mod live_component;

#[cfg(feature = "preview-session")]
pub mod preview_sessions;

#[cfg(feature = "preview-session")]
pub mod inspector;

#[cfg(feature = "preview-session")]
pub mod debug_agent;

#[cfg(feature = "remote")]
pub mod remote;

#[cfg(feature = "serial")]
pub mod serial;
