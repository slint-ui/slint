// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The query-based formatter modules are only reachable through `tool`, which
// is not compiled for wasm32. TODO: remove the dead_code allowances once the
// query formatter is also wired to the LSP formatting path.
#[allow(dead_code)]
pub mod atoms;
#[allow(dead_code)]
pub mod engine;
#[allow(clippy::module_inception)]
pub mod fmt;
#[allow(dead_code)]
pub mod render;
#[allow(dead_code)]
pub mod rules;
#[cfg(not(target_arch = "wasm32"))]
pub mod tool;
pub mod writer;
