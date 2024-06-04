// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains helper functions that are used for our internal tests within Slint

use crate::TestingWindow;
use i_slint_core::api::ComponentHandle;
use i_slint_core::platform::WindowEvent;
pub use i_slint_core::tests::slint_get_mocked_time as get_mocked_time;
pub use i_slint_core::tests::slint_mock_elapsed_time as mock_elapsed_time;
use i_slint_core::window::WindowInner;
use i_slint_core::SharedString;

/// Simulate a mouse click
pub fn send_mouse_click<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable> + 'static,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    x: f32,
    y: f32,
) {
    i_slint_core::tests::slint_send_mouse_click(
        x,
        y,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate entering a sequence of ascii characters key by (pressed or released).
pub fn send_keyboard_char<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    string: char,
    pressed: bool,
) {
    i_slint_core::tests::slint_send_keyboard_char(
        &SharedString::from(string),
        pressed,
        &WindowInner::from_pub(component.window()).window_adapter(),
    )
}

/// Simulate entering a sequence of ascii characters key by key.
pub fn send_keyboard_string_sequence<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    sequence: &str,
) {
    i_slint_core::tests::send_keyboard_string_sequence(
        &SharedString::from(sequence),
        &WindowInner::from_pub(component.window()).window_adapter(),
    )
}

/// Applies the specified scale factor to the window that's associated with the given component.
/// This overrides the value provided by the windowing system.
pub fn set_window_scale_factor<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    factor: f32,
) {
    component.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: factor });
}

pub fn access_testing_window<R>(
    window: &i_slint_core::api::Window,
    callback: impl FnOnce(&TestingWindow) -> R,
) -> R {
    i_slint_core::window::WindowInner::from_pub(window)
        .window_adapter()
        .internal(i_slint_core::InternalToken)
        .and_then(|wa| wa.as_any().downcast_ref::<TestingWindow>())
        .map(callback)
        .expect("access_testing_window called without testing backend/adapter")
}
