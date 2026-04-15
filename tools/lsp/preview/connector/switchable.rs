use i_slint_preview_protocol::PreviewTarget;
use std::{any::Any, cell::RefCell, collections::HashMap};

use crate::common::{LspToPreview, Result};

pub struct SwitchableLspToPreview {
    lsp_to_previews: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
    current_target: RefCell<PreviewTarget>,
}

impl SwitchableLspToPreview {
    pub fn new(
        lsp_to_previews: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
        current_target: PreviewTarget,
    ) -> Result<Self> {
        if lsp_to_previews.contains_key(&current_target) {
            Ok(Self { lsp_to_previews, current_target: RefCell::new(current_target) })
        } else {
            Err("No such target".into())
        }
    }

    #[allow(unused)]
    pub fn with_one(lsp_to_preview: impl LspToPreview) -> Self {
        let target = lsp_to_preview.preview_target();
        let lsp_to_previews =
            std::iter::once((target, Box::new(lsp_to_preview) as Box<dyn LspToPreview>)).collect();
        Self { lsp_to_previews, current_target: RefCell::new(target) }
    }

    pub fn send(&self, message: &i_slint_preview_protocol::LspToPreviewMessage) {
        self.lsp_to_previews.get(&self.current_target.borrow()).unwrap().send(message);
    }

    pub async fn shutdown(&self) {
        self.send(&i_slint_preview_protocol::LspToPreviewMessage::Quit);
        futures_util::future::join_all(
            self.lsp_to_previews.values().map(|to_preview| to_preview.shutdown()),
        )
        .await;
    }

    #[allow(unused)]
    pub fn preview_target(&self) -> PreviewTarget {
        *self.current_target.borrow()
    }

    pub fn set_preview_target(&self, target: PreviewTarget) -> Result<()> {
        if self.lsp_to_previews.contains_key(&target) {
            *self.current_target.borrow_mut() = target;
            Ok(())
        } else {
            Err("Target not found".into())
        }
    }

    #[allow(unused)]
    pub fn with_preview_target<T: LspToPreview, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        for target in self.lsp_to_previews.values() {
            if let Some(target) = <dyn Any>::downcast_ref(target.as_ref()) {
                return Some(f(target));
            }
        }
        None
    }

    #[allow(unused)]
    pub async fn with_preview_target_async<T: LspToPreview, R>(
        &self,
        f: impl AsyncFnOnce(&T) -> R,
    ) -> Option<R> {
        for target in self.lsp_to_previews.values() {
            if let Some(target) = <dyn Any>::downcast_ref(target.as_ref()) {
                return Some(f(target).await);
            }
        }
        None
    }
}
