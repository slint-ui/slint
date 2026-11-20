// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::{Rc, Weak};

use accesskit::ActionRequest;
use i_slint_core::accessibility::accesskit_tree::{
    AccessKitHost, AccessKitTree, AccessKitUpdateState, DeferredAccessKitAction,
};
use i_slint_core::item_tree::ItemTreeRef;

use super::WinitWindowAdapter;
use crate::SlintEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

/// The AccessKit adapter connects the given window adapter's accesskit node tree, built by
/// [`AccessKitTree`], with the platform through `accesskit_winit`.
///
/// For unix it's necessary to inform accesskit about any changes to the position or size of the window, hence
/// the `on_event` function that needs calling.
///
/// Similarly, when the window adapter is informed about a focus change, handle_focus_change must be called.
/// Finally, when a component is destroyed, `unregister_item_tree` must be called, which rebuilds the entire
/// tree at the moment.
///
/// If we wanted to move this to corelib, `on_event` gets replaced with listening to the events sent from the
/// platform adapter to the slint::Window. `handle_focus_change` is already internal to WindowInner, as well
/// as `component_destroyed`. The `WindowInner` would own this `AccessKit`.
pub struct AccessKitAdapter {
    inner: accesskit_winit::Adapter,
    window_adapter_weak: Weak<WinitWindowAdapter>,
    tree: AccessKitTree,
    update_state: AccessKitUpdateState,
}

impl AccessKitAdapter {
    pub fn new(
        window_adapter_weak: Weak<WinitWindowAdapter>,
        active_event_loop: &ActiveEventLoop,
        winit_window: &winit::window::Window,
        proxy: EventLoopProxy<SlintEvent>,
    ) -> Self {
        Self {
            inner: accesskit_winit::Adapter::with_event_loop_proxy(
                active_event_loop,
                winit_window,
                proxy,
            ),
            window_adapter_weak: window_adapter_weak.clone(),
            tree: AccessKitTree::new(
                window_adapter_weak.clone(),
                Rc::new(WinitAccessKitHost { window_adapter_weak }),
            ),
            update_state: Default::default(),
        }
    }

    pub fn process_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        if matches!(event, winit::event::WindowEvent::Focused(_)) {
            self.tree.set_dirty();
            self.invoke_later(|self_cell, _| self_cell.borrow_mut().rebuild_tree_of_dirty_nodes());
        }
        self.inner.process_event(window, event);
    }

    pub fn process_accesskit_event(
        &mut self,
        window_event: accesskit_winit::WindowEvent,
    ) -> Option<DeferredAccessKitAction> {
        match window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.inner.update_if_active(|| self.tree.build_new_tree());
                self.update_state.set_initial_tree_sent();
                None
            }
            accesskit_winit::WindowEvent::ActionRequested(r) => self.handle_request(r),
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.update_state.set_deactivated();
                self.tree.clear_text_state();
                None
            }
        }
    }

    pub fn handle_focus_item_change(&mut self) {
        if !self.update_state.should_send_focus() {
            return;
        }
        self.inner.update_if_active(|| self.tree.focus_update())
    }

    fn handle_request(&self, request: ActionRequest) -> Option<DeferredAccessKitAction> {
        self.tree.handle_request(request)
    }

    pub fn reload_tree(&mut self) {
        if !self.update_state.should_schedule_reload() {
            return;
        }

        self.invoke_later(|self_cell, _| {
            let mut self_ = self_cell.borrow_mut();
            let self_ = &mut *self_;
            if self_.update_state.reload_started() {
                self_.inner.update_if_active(|| self_.tree.build_new_tree())
            }
        });
    }

    pub fn unregister_item_tree(&mut self, component: ItemTreeRef) {
        self.tree.forget_item_tree(component);
        self.reload_tree();
    }

    fn rebuild_tree_of_dirty_nodes(&mut self) {
        if !self.update_state.should_send_dirty_nodes(&self.tree) {
            return;
        }

        self.inner.update_if_active(|| self.tree.rebuild_dirty_nodes())
    }

    fn invoke_later(
        &self,
        callback: impl FnOnce(&std::cell::RefCell<Self>, Weak<WinitWindowAdapter>) + 'static,
    ) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            WinitWindowAdapter::with_access_kit_adapter_from_weak_window_adapter(
                win.clone(),
                move |self_cell| callback(self_cell, win),
            );
        });
    }
}

struct WinitAccessKitHost {
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl WinitAccessKitHost {
    fn with_adapter_later(&self, callback: impl FnOnce(&mut AccessKitAdapter) + 'static) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            WinitWindowAdapter::with_access_kit_adapter_from_weak_window_adapter(
                win,
                |self_cell| callback(&mut self_cell.borrow_mut()),
            );
        })
    }
}

impl AccessKitHost for WinitAccessKitHost {
    fn window_has_focus(&self) -> bool {
        self.window_adapter_weak
            .upgrade()
            .and_then(|window_adapter| window_adapter.winit_window())
            .is_some_and(|winit_window| winit_window.has_focus())
    }

    fn nodes_dirty(&self) {
        self.with_adapter_later(|adapter| adapter.rebuild_tree_of_dirty_nodes());
    }

    fn focus_dirty(&self) {
        self.with_adapter_later(|adapter| adapter.handle_focus_item_change());
    }
}

impl From<accesskit_winit::Event> for SlintEvent {
    fn from(value: accesskit_winit::Event) -> Self {
        SlintEvent(crate::event_loop::CustomEvent::Accesskit(value))
    }
}
