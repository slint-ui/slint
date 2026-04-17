// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("../README.md")]

mod lsp_to_preview;
mod preview_to_lsp;
mod versioned_url;

pub use lsp_to_preview::{LspToPreviewMessage, PreviewComponent, PreviewConfig};
pub use preview_to_lsp::{PreviewTarget, PreviewToLspMessage};
pub use versioned_url::VersionedUrl;

pub use lsp_types;

pub type SourceFileVersion = Option<i32>;
pub const SERVICE_TYPE: &str = "_slint-preview._tcp.local.";
