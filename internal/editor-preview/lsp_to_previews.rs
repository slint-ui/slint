// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{LspToPreview, Result};

/// An `LspToPreview` that drops every message.
#[derive(Default, Clone)]
pub struct DummyLspToPreview {}

impl LspToPreview for DummyLspToPreview {
    fn send(&self, _message: &LspToPreviewMessage) {}

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::Dummy
    }
}

/// The connection to a remote viewer. The transport implementing this lives in
/// the LSP binary's `connector` module: applications that never talk to a
/// remote viewer do not build it.
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
pub trait RemoteTransport {
    fn send(&self, message: &LspToPreviewMessage);
    fn connect(
        &self,
        addresses: Vec<String>,
        port: u16,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<()>>>>;
    fn disconnect(&self) -> std::pin::Pin<Box<dyn Future<Output = ()>>>;
    /// The code the user read off the viewer's screen.
    fn submit_pairing_code(&self, code: String);
    /// Abandon the pairing attempt the user was prompted for.
    fn cancel_pairing(&self);
    /// Connect to the pairing-disabled viewer the user was warned about.
    fn accept_unpaired_connection(&self);
}

#[cfg(feature = "system-testing")]
type SourceObserver = Rc<dyn Fn(&lsp_types::Url, Option<&str>)>;

/// Fans LSP messages out to the active local preview and, if connected, to a
/// remote viewer. The local target is itself swappable between `ChildProcess`
/// and `EmbeddedWasm`, driven by
/// [`PreviewTypeChanged`](i_slint_live_preview::protocol::PreviewToLspMessage::PreviewTypeChanged).
/// The remote viewer receives every wire-format message in parallel — it isn't
/// a target on its own.
pub struct LspToPreviews {
    #[cfg(feature = "system-testing")]
    source_observer: RefCell<Option<SourceObserver>>,
    locals: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
    current_local: RefCell<PreviewTarget>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    remote: Option<Rc<dyn RemoteTransport>>,
}

#[allow(dead_code)] // Which methods are live depends on the enabled preview features.
impl LspToPreviews {
    pub fn new(
        locals: HashMap<PreviewTarget, Box<dyn LspToPreview>>,
        current_local: PreviewTarget,
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        make_remote: impl FnOnce(std::rc::Weak<Self>) -> Rc<dyn RemoteTransport>,
    ) -> Result<Rc<Self>> {
        if !locals.contains_key(&current_local) {
            return Err("No such target".into());
        }
        // `new_cyclic` hands the not-yet-constructed `Self` a `Weak` to
        // itself, which the remote transport keeps for the connection-state
        // back-channel without forming an `Rc` cycle.
        Ok(Rc::new_cyclic(|_weak| Self {
            #[cfg(feature = "system-testing")]
            source_observer: Default::default(),
            locals,
            current_local: RefCell::new(current_local),
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            remote: Some(make_remote(_weak.clone())),
        }))
    }

    pub fn with_one(lsp_to_preview: impl LspToPreview + 'static) -> Rc<Self> {
        let target = lsp_to_preview.preview_target();
        let locals =
            std::iter::once((target, Box::new(lsp_to_preview) as Box<dyn LspToPreview>)).collect();
        Rc::new(Self {
            #[cfg(feature = "system-testing")]
            source_observer: Default::default(),
            locals,
            current_local: RefCell::new(target),
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            remote: None,
        })
    }

    #[cfg(feature = "system-testing")]
    pub fn set_source_observer(&self, observer: SourceObserver) {
        self.source_observer.replace(Some(observer));
    }

    #[cfg(feature = "system-testing")]
    pub(crate) fn observe_source(&self, url: &lsp_types::Url, content: Option<&str>) {
        if let Some(observer) = self.source_observer.borrow().as_ref() {
            observer(url, content);
        }
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
    pub fn remote(&self) -> Option<&Rc<dyn RemoteTransport>> {
        self.remote.as_ref()
    }
}
