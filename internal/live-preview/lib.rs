// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

#[cfg(feature = "protocol")]
pub mod protocol;

#[cfg(feature = "file-watcher")]
pub mod file_watcher;

#[cfg(feature = "live-component")]
pub mod live_component;
