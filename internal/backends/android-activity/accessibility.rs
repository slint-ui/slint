// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Exposes the accessible tree of the window to TalkBack and other accessibility services,
//! with the help of [AccessKit](https://accesskit.dev).
//!
//! The Android accessibility framework calls into the application on the UI thread of the
//! activity, while all Slint objects live on the thread that runs the event loop. The handlers
//! below therefore never touch Slint objects: they send the request to the event loop thread,
//! which is also the only thread that builds and updates the tree.

use crate::androidwindowadapter::AndroidWindowAdapter;
use crate::javahelper::JavaHelper;
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, Role, TreeUpdate};
use accesskit_android::InjectingAdapter;
use i_slint_core::accessibility::accesskit_tree::{AccessKitHost, AccessKitTree};
use i_slint_core::api::invoke_from_event_loop;
use i_slint_core::platform::WindowAdapter;
use i_slint_core::timers::Timer;
use i_slint_core::window::WindowInner;
use std::rc::{Rc, Weak};
use std::sync::atomic::AtomicBool;

/// Whether touch exploration is on, which is when hover events are for the accessibility adapter.
pub(crate) static TOUCH_EXPLORATION: AtomicBool = AtomicBool::new(false);

/// Connects the node tree built by [`AccessKitTree`] with the Android accessibility framework.
pub(crate) struct Accessibility {
    adapter: InjectingAdapter,
    tree: AccessKitTree,
    window_adapter_weak: Weak<AndroidWindowAdapter>,
    /// A rebuild of the whole tree is scheduled.
    pending_update: bool,
    /// An accessibility service asked for the tree, which was sent.
    initial_tree_sent: bool,
    /// The window origin relative to the hosting view of the tree that was sent last.
    origin_in_view: (i32, i32),
    /// Whether the origin is going to be compared.
    pending_origin_check: bool,
    /// Whether an accessibility service is enabled. The tree is not kept up to date while none is.
    enabled: bool,
}

impl Accessibility {
    /// Installs the accessibility delegate on the view that covers the activity.
    pub(crate) fn new(
        window_adapter_weak: Weak<AndroidWindowAdapter>,
        java_helper: &JavaHelper,
    ) -> Result<Self, String> {
        // accesskit_android panics when it cannot load its Java class or create its delegate.
        // This does not help when the application is built with `panic = "abort"`.
        let adapter = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            java_helper.install_accessibility_adapter(Activation, Actions)
        }))
        .map_err(|_| "the AccessKit adapter panicked".to_string())?
        .map_err(|e| format!("{e:#?}"))?
        .ok_or("the view has an adapter already")?;
        let tree = AccessKitTree::new(
            window_adapter_weak.clone() as Weak<dyn WindowAdapter>,
            Rc::new(Host { window_adapter_weak: window_adapter_weak.clone() }),
        );
        Ok(Self {
            adapter,
            tree,
            window_adapter_weak,
            pending_update: false,
            initial_tree_sent: false,
            origin_in_view: (0, 0),
            pending_origin_check: false,
            enabled: true,
        })
    }

    /// Sends the whole tree. Nothing is built before an accessibility service asks for the tree.
    fn send_tree(&mut self, origin_in_view: (i32, i32)) {
        self.origin_in_view = origin_in_view;
        self.tree.set_physical_offset(origin_in_view.0 as f64, origin_in_view.1 as f64);
        self.adapter.update_if_active(|| without_window_label(self.tree.build_new_tree()));
        self.initial_tree_sent = true;
    }

    /// A service has the tree and listens to the updates.
    fn is_live(&self) -> bool {
        self.initial_tree_sent && self.enabled
    }

    /// Sends the whole tree again, after the structure of the items changed or the window moved
    /// relative to the view that hosts the adapter.
    fn reload_tree(&mut self) {
        if self.pending_update || !self.initial_tree_sent {
            return;
        }
        self.pending_update = true;
        self.run_later(|this| {
            this.pending_update = false;
            if this.enabled
                && let Some(window_adapter) = this.window_adapter_weak.upgrade()
            {
                this.send_tree(window_adapter.accessibility_origin_in_view());
            }
        });
    }

    /// Sends the tree again when the window moved relative to the view that hosts the adapter,
    /// which happens with the insets or the keyboard, without a resize of the window.
    fn check_origin(&mut self) {
        if self.pending_origin_check || !self.is_live() {
            return;
        }
        self.pending_origin_check = true;
        self.run_later(|this| {
            this.pending_origin_check = false;
            if this.is_live()
                && let Some(window_adapter) = this.window_adapter_weak.upgrade()
            {
                let origin = window_adapter.accessibility_origin_in_view();
                if origin != this.origin_in_view {
                    this.send_tree(origin);
                }
            }
        });
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            // The tree was not kept up to date while no service was enabled
            self.reload_tree();
        } else {
            self.tree.clear_text_state();
        }
    }

    fn rebuild_dirty_nodes(&mut self) {
        // A pending reload sends everything anyway, and the focus node might not be known to
        // Android yet when the structure of the tree changed.
        if !self.is_live() || self.pending_update || !self.tree.has_dirty_nodes() {
            return;
        }
        self.adapter.update_if_active(|| without_window_label(self.tree.rebuild_dirty_nodes()));
    }

    fn update_focus(&mut self) {
        if !self.is_live() || self.pending_update {
            return;
        }
        self.adapter.update_if_active(|| self.tree.focus_update());
    }

    /// Runs `callback` later, outside of the property system and of any borrow.
    fn run_later(&self, callback: impl FnOnce(&mut Accessibility) + 'static) {
        run_with_accessibility_later(self.window_adapter_weak.clone(), callback);
    }
}

/// Takes the label off the node of the window.
///
/// Android names the window after the activity. With a label of its own, the node of the window
/// is a stop for screen readers, which land on it when a touch hits no element and then frame the
/// whole window.
fn without_window_label(mut update: TreeUpdate) -> TreeUpdate {
    for (_, node) in &mut update.nodes {
        if node.role() == Role::Window {
            node.clear_label();
        }
    }
    update
}

fn run_with_accessibility_later(
    window_adapter_weak: Weak<AndroidWindowAdapter>,
    callback: impl FnOnce(&mut Accessibility) + 'static,
) {
    Timer::single_shot(Default::default(), move || {
        if let Some(window_adapter) = window_adapter_weak.upgrade() {
            window_adapter.with_accessibility(callback);
        }
    });
}

impl AndroidWindowAdapter {
    fn with_accessibility(&self, callback: impl FnOnce(&mut Accessibility)) {
        // Items created or destroyed while the tree is built notify us while it is borrowed. The
        // tree takes them into account anyway.
        if let Ok(mut accessibility) = self.accessibility.try_borrow_mut()
            && let Some(accessibility) = accessibility.as_mut()
        {
            callback(accessibility);
        }
    }

    /// Where the window content starts relative to the view that hosts the accessibility delegate.
    fn accessibility_origin_in_view(&self) -> (i32, i32) {
        let (x, y) = self.java_helper.window_origin_in_view().unwrap_or_else(|e| {
            i_slint_core::debug_log!("Getting the position of the window failed: {e:#?}");
            Default::default()
        });
        let offset = self.offset.get();
        (x + offset.x, y + offset.y)
    }

    /// An accessibility service asked for the tree for the first time.
    fn accessibility_activated(&self) {
        let origin = self.accessibility_origin_in_view();
        self.with_accessibility(|a| a.send_tree(origin));
    }

    fn accessibility_action(&self, request: ActionRequest) {
        let deferred = self
            .accessibility
            .try_borrow()
            .ok()
            .and_then(|a| a.as_ref().and_then(|a| a.tree.handle_request(request)));
        if let Some(deferred) = deferred {
            deferred.invoke(&self.window);
        }
    }

    /// The items were created or destroyed, or the window moved relative to the hosting view.
    pub(crate) fn accessibility_reload_tree(&self) {
        self.with_accessibility(|a| a.reload_tree());
    }

    /// The window may have moved relative to the view that hosts the accessibility delegate.
    pub(crate) fn accessibility_check_origin(&self) {
        self.with_accessibility(|a| a.check_origin());
    }

    /// An accessibility service was enabled or disabled.
    pub(crate) fn accessibility_state_changed(&self, enabled: bool) {
        self.with_accessibility(|a| a.set_enabled(enabled));
    }

    pub(crate) fn accessibility_forget_item_tree(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
    ) {
        self.with_accessibility(|a| a.tree.forget_item_tree(component));
        self.accessibility_reload_tree();
    }

    pub(crate) fn accessibility_focus_item_changed(&self) {
        self.with_accessibility(|a| a.update_focus());
    }

    /// The window gained or lost the focus, which the node that is reported as focused depends on.
    pub(crate) fn accessibility_window_focus_changed(&self) {
        self.with_accessibility(|a| {
            a.tree.set_dirty();
            a.run_later(|a| a.rebuild_dirty_nodes());
        });
    }
}

struct Host {
    window_adapter_weak: Weak<AndroidWindowAdapter>,
}

impl AccessKitHost for Host {
    fn window_has_focus(&self) -> bool {
        self.window_adapter_weak
            .upgrade()
            .is_some_and(|adapter| WindowInner::from_pub(&adapter.window).active())
    }

    fn nodes_dirty(&self) {
        run_with_accessibility_later(self.window_adapter_weak.clone(), |a| a.rebuild_dirty_nodes());
    }

    fn focus_dirty(&self) {
        run_with_accessibility_later(self.window_adapter_weak.clone(), |a| a.update_focus());
    }
}

/// Runs `f` on the event loop thread with the window adapter, if there is one.
fn with_window_adapter(f: impl FnOnce(&Rc<AndroidWindowAdapter>) + Send + 'static) {
    let _ = invoke_from_event_loop(move || {
        if let Some(adapter) = crate::CURRENT_WINDOW.with_borrow(|w| w.upgrade()) {
            f(&adapter);
        }
    });
}

struct Activation;

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // This is called on the UI thread when a service asks for the tree for the first time.
        // Until the event loop thread has sent the tree, AccessKit serves a placeholder.
        with_window_adapter(|adapter| adapter.accessibility_activated());
        None
    }
}

struct Actions;

impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        with_window_adapter(move |adapter| adapter.accessibility_action(request));
    }
}
