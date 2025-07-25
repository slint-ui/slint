// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

use crate::{common, preview};

pub fn lsp_to_preview(message: common::LspToPreviewMessage) {
    use common::LspToPreviewMessage as M;
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
