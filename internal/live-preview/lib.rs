// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

/// Window over which a burst of keystrokes from the editor is coalesced into a single
/// preview rebuild. Used by both the in-process LSP preview and the remote viewer so a
/// single value governs how reactive the preview feels.
pub const REBUILD_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(50);

/// Build a `data:` URL that embeds `bytes`, choosing the media type from the file
/// `extension` (falling back to `application/octet-stream` for unknown types). Used to hand a
/// resource to a renderer that can not reach the original file: the remote viewer over its
/// connection, or SlintPad loading a shared gist.
pub fn make_data_url(extension: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    let mime = match extension.to_ascii_lowercase().as_str() {
        "svg" | "svgz" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg(feature = "protocol")]
pub mod protocol;

#[cfg(feature = "file-watcher")]
pub mod file_watcher;

#[cfg(feature = "live-component")]
pub mod live_component;

#[cfg(feature = "remote")]
pub mod remote;
