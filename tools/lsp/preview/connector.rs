// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
use std::{cell::RefCell, collections::HashMap};

#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

#[cfg(feature = "preview-remote")]
pub mod remote;
#[cfg(feature = "preview-remote")]
pub use remote::*;

use crate::{common, preview};

pub fn lsp_to_preview(message: common::LspToPreviewMessage) {
    use common::LspToPreviewMessage as M;
    match message {
        M::InvalidateContents { url } => preview::invalidate_contents(&url),
        M::ForgetFile { url } => preview::delete_document(&url),
        M::SetContents { url, contents } => {
            if let Ok(contents) = String::from_utf8(contents) {
                preview::set_contents(&url, contents);
            }
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
        M::Quit => {
            #[cfg(not(target_arch = "wasm32"))]
            let _ = slint::quit_event_loop();
        }
    }
}

pub struct SwitchableLspToPreview {
    lsp_to_previews: HashMap<common::PreviewTarget, Box<dyn common::LspToPreview>>,
    current_target: RefCell<common::PreviewTarget>,
}

impl SwitchableLspToPreview {
    pub fn new(
        lsp_to_previews: HashMap<common::PreviewTarget, Box<dyn common::LspToPreview>>,
        current_target: common::PreviewTarget,
    ) -> common::Result<Self> {
        if lsp_to_previews.contains_key(&current_target) {
            Ok(Self { lsp_to_previews, current_target: RefCell::new(current_target) })
        } else {
            Err("No such target".into())
        }
    }

    pub fn send(&self, message: &common::LspToPreviewMessage) {
        self.lsp_to_previews.get(&self.current_target.borrow()).unwrap().send(message);
    }

    #[allow(unused)]
    pub fn with_preview_target<T: common::LspToPreview, R>(
        &self,
        f: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        for target in self.lsp_to_previews.values() {
            if let Some(target) = target.as_any().downcast_ref::<T>() {
                return Some(f(target));
            }
        }
        None
    }

    pub async fn with_preview_target_async<T: common::LspToPreview, R>(
        &self,
        f: impl AsyncFnOnce(&T) -> R,
    ) -> Option<R> {
        for target in self.lsp_to_previews.values() {
            if let Some(target) = target.as_any().downcast_ref::<T>() {
                return Some(f(target).await);
            }
        }
        None
    }

    pub fn set_preview_target(&self, target: common::PreviewTarget) -> common::Result<()> {
        if self.lsp_to_previews.contains_key(&target) {
            *self.current_target.borrow_mut() = target;
            Ok(())
        } else {
            Err("Target not found".into())
        }
    }
}
