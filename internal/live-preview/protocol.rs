// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod lsp_to_preview;
mod preview_to_lsp;
mod versioned_url;

pub use lsp_to_preview::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, RemoteConnectionState,
};
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

/// Wire-format identifier sent in the `Sec-WebSocket-Protocol` header at
/// handshake time. Built from the Slint `MAJOR.MINOR` version: a 1.17
/// viewer never talks to a 1.18 LSP. Bump happens automatically on every
/// minor release. Patch releases keep the same identifier.
pub const PROTOCOL_SUBPROTOCOL: &str = concat!(
    "slint-preview.",
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR"),
);

/// Full Slint version of this build, shown in the discovery picker and in
/// version-mismatch error messages.
pub const SLINT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// HTTP header set on every WebSocket handshake response (success or
/// failure) so the LSP can report the viewer's actual version even when
/// the subprotocol does not match.
pub const SLINT_VERSION_HEADER: &str = "Slint-Version";

/// HTTP header carrying a comma-separated list of subprotocols the
/// server accepts. Set on every handshake response.
pub const SLINT_PROTOCOLS_HEADER: &str = "Slint-Protocols";

/// mDNS TXT record key for the comma-separated list of subprotocols a
/// viewer advertises support for.
pub const TXT_PROTOCOLS_KEY: &str = "protocols";

/// mDNS TXT record key for the full Slint version of the viewer.
pub const TXT_SLINT_VERSION_KEY: &str = "slint";
