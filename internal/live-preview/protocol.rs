// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod lsp_to_preview;
mod preview_to_lsp;
mod versioned_url;

pub use lsp_to_preview::{LspToPreviewMessage, PreviewComponent, PreviewConfig};
pub use preview_to_lsp::{PreviewTarget, PreviewToLspMessage};
pub use versioned_url::VersionedUrl;

pub use lsp_types;

#[cfg(feature = "file-watcher")]
mod diagnostics_adapter;
#[cfg(feature = "file-watcher")]
pub use diagnostics_adapter::to_lsp_diagnostic;

pub type SourceFileVersion = Option<i32>;
pub const SERVICE_TYPE: &str = "_slint-preview._tcp.local.";
pub const SERVICE_TYPE_NAME: &str = "slint-preview";
pub const SERVICE_TYPE_PROTOCOL: &str = "tcp";
