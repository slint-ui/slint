// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

/// Window over which a burst of keystrokes from the editor is coalesced into a single
/// preview rebuild. Used by both the in-process LSP preview and the remote viewer so a
/// single value governs how reactive the preview feels.
pub const REBUILD_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(50);

#[cfg(feature = "protocol")]
pub mod protocol;

#[cfg(feature = "file-watcher")]
pub mod file_watcher;

#[cfg(feature = "live-component")]
pub mod live_component;

#[cfg(feature = "remote")]
pub mod remote;
