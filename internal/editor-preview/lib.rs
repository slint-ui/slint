// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![deny(clippy::print_stderr, clippy::print_stdout, clippy::disallowed_methods)]

pub mod component_catalog;
pub mod document_cache;
pub mod editing;
pub mod editor_session;
pub mod element;
pub mod file_url;
mod lsp_to_previews;
#[cfg(not(target_arch = "wasm32"))]
pub mod settings_store;
#[cfg(any(test, feature = "testing"))]
pub mod test;
pub mod token_info;
pub mod util;

pub use document_cache::DocumentCache;
pub use editor_session::{EditorSession, VersionedDiagnostics};
pub use element::{ElementRcNode, NODE_IGNORE_COMMENT, extract_element, is_element_node_ignored};
pub use file_url::{file_to_uri, uri_to_file};
pub use i_slint_compiler::diagnostics::ByteFormat;
#[cfg(target_arch = "wasm32")]
pub use i_slint_live_preview::protocol::wasm_prelude;
pub use i_slint_live_preview::protocol::{LspToPreview, PreviewToLsp, Result};
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
pub use lsp_to_previews::RemoteTransport;
pub use lsp_to_previews::{DummyLspToPreview, LspToPreviews};

#[allow(clippy::disallowed_methods)]
pub fn spawn_local<Future>(future: Future)
where
    Future: std::future::Future + 'static,
    Future::Output: 'static,
{
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let _ = future.await;
    });
    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn_local(future);
}

/// Converts a log message from the preview to a string to be logged by the LSP.
pub fn preview_log_message_to_string(
    location: &Option<(std::path::PathBuf, usize, usize)>,
    message: &str,
) -> String {
    if let Some((file, line, column)) = location {
        format!("DEBUG {file}:{line}:{column}> {message}", file = file.display())
    } else {
        format!("DEBUG> {message}")
    }
}
