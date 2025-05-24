// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is what connects the live-preview part to the outside world.
//!
//! This file is tricky: Most functions may be called from a different thread!

#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

use crate::{common, preview};

fn lsp_to_preview_message_impl(message: crate::common::LspToPreviewMessage) {
    use crate::common::LspToPreviewMessage as M;
    match message {
        M::InvalidateContents { url } => preview::invalidate_contents(&url),
        M::ForgetFile { url } => preview::delete_document(&url),
        M::SetContents { url, contents } => {
            preview::set_contents(&url, contents);
        }
        M::SetConfiguration { config } => {
            preview::config_changed(config);
        }
        M::ShowPreview(pc) => {
            preview::load_preview(pc, preview::LoadBehavior::BringWindowToFront);
        }
        M::HighlightFromEditor { url, offset } => {
            preview::highlight(url, offset.into());
        }
    }
}

/// Sends a notification back to the editor when the preview fails to load because of a slint::PlatformError.
pub fn send_platform_error_notification(platform_error_str: &str) {
    let message = format!("Error displaying the Slint preview window: {platform_error_str}");
    // Also output the message in the console in case the user missed the notification in the editor
    eprintln!("{message}");
    send_message_to_lsp(common::PreviewToLspMessage::SendShowMessage {
        message: lsp_types::ShowMessageParams { typ: lsp_types::MessageType::ERROR, message },
    })
}

/// Sends a telemetry event
pub fn send_telemetry(data: &mut [(String, serde_json::Value)]) {
    let object = {
        let mut object = serde_json::Map::new();
        for (name, value) in data.iter_mut() {
            object.insert(std::mem::take(name), std::mem::take(value));
        }
        object
    };
    send_message_to_lsp(crate::common::PreviewToLspMessage::TelemetryEvent(object));
}

/// Run a callback when the UI is opened
///
/// This happens in the UI thread
#[cfg(not(target_arch = "wasm32"))]
pub fn open_ui_callback(
    preview_state: &mut preview::PreviewState,
) -> Result<(), slint::PlatformError> {
    native::open_ui_impl(preview_state)
}
#[cfg(target_arch = "wasm32")]
pub fn open_ui_callback(
    preview_state: &mut preview::PreviewState,
) -> Result<(), slint::PlatformError> {
    Ok(())
}
