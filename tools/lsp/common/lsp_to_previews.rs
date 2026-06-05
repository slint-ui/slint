// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::{LspToPreview, Result};
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
use {
    crate::preview::connector::remote::RemoteLspToPreview,
    i_slint_live_preview::protocol::PreviewToLspMessage, tokio::sync::mpsc,
};

/// Fans LSP messages out to the active local preview and, if connected,
/// to a remote viewer. The local target is itself swappable between
/// `ChildProcess` and `EmbeddedWasm` (driven by
/// [`PreviewToLspMessage::PreviewTypeChanged`]). The remote viewer
/// receives every wire-format message in parallel — it isn't a target on
/// its own.
pub struct LspToPreviews {
    locals: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
    current_local: RefCell<PreviewTarget>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    remote: Option<Rc<RemoteLspToPreview>>,
}

#[allow(dead_code)] // Which methods are live depends on the enabled preview features.
impl LspToPreviews {
    pub fn new(
        locals: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
        current_local: PreviewTarget,
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
    ) -> Result<Rc<Self>> {
        if !locals.contains_key(&current_local) {
            return Err("No such target".into());
        }
        // `new_cyclic` hands the not-yet-constructed `Self` a `Weak` to
        // itself, which `RemoteLspToPreview` keeps for the connection-state
        // back-channel without forming an `Rc` cycle.
        Ok(Rc::new_cyclic(|_weak| Self {
            locals,
            current_local: RefCell::new(current_local),
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            remote: Some(Rc::new(RemoteLspToPreview::new(preview_to_lsp_sender, _weak.clone()))),
        }))
    }

    pub fn with_one(lsp_to_preview: impl LspToPreview) -> Rc<Self> {
        let target = lsp_to_preview.preview_target();
        let locals =
            std::iter::once((target, Box::new(lsp_to_preview) as Box<dyn LspToPreview>)).collect();
        Rc::new(Self {
            locals,
            current_local: RefCell::new(target),
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            remote: None,
        })
    }

    /// Send to the local preview and to the remote viewer in parallel.
    /// Both render the same content; the local one shows a placeholder
    /// while remote is active so it isn't wasting CPU on a stale render.
    pub fn send(&self, message: &LspToPreviewMessage) {
        self.locals.get(&self.current_local.borrow()).unwrap().send(message);
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        if let Some(remote) = &self.remote {
            remote.send(message);
        }
    }

    /// Send only to the local preview. Used for control messages (e.g.
    /// `RemoteConnectionState`) that must reach the dialog and never the
    /// remote viewer.
    pub fn send_to_local_preview(&self, message: &LspToPreviewMessage) {
        self.locals.get(&self.current_local.borrow()).unwrap().send(message);
    }

    pub async fn shutdown(&self) {
        // Quit only goes to the local previews; the remote viewer is an
        // independent process that should outlive this LSP session.
        for to_preview in self.locals.values() {
            to_preview.send(&LspToPreviewMessage::Quit);
        }
        futures_util::future::join_all(
            self.locals.values().map(|to_preview| to_preview.shutdown()),
        )
        .await;
    }

    pub fn local_target(&self) -> PreviewTarget {
        *self.current_local.borrow()
    }

    pub fn set_local_target(&self, target: PreviewTarget) -> Result<()> {
        if self.locals.contains_key(&target) {
            *self.current_local.borrow_mut() = target;
            Ok(())
        } else {
            Err("Target not found".into())
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    pub fn remote(&self) -> Option<&Rc<RemoteLspToPreview>> {
        self.remote.as_ref()
    }
}
