// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub mod atoms;
pub mod engine;
// The old imperative formatter: dead code, kept only until its tests are
// ported to the query-based formatter.
#[allow(dead_code)]
#[allow(clippy::module_inception)]
pub mod fmt;
pub mod render;
pub mod rules;
#[cfg(test)]
mod tests;
#[cfg(not(target_arch = "wasm32"))]
pub mod tool;
pub mod writer;
