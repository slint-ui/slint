// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains helper functions that are used for our internal tests within Slint

use crate::TestingWindow;
use i_slint_core::SharedString;
use i_slint_core::api::ComponentHandle;
use i_slint_core::platform::WindowEvent;
pub use i_slint_core::tests::slint_get_mocked_time as get_mocked_time;
pub use i_slint_core::tests::slint_mock_elapsed_time as mock_elapsed_time;
use i_slint_core::window::WindowInner;

/// Simulate a mouse click at `(x, y)` and release after a while at the same position
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

/// Simulate IME preedit (composition) text input.
///
/// This simulates the behavior of an IME setting preedit text on the focused TextInput.
/// The preedit text is displayed but not yet committed to the text field.
///
/// # Arguments
/// * `component` - The component containing the focused TextInput
/// * `preedit` - The preedit/composition text to display (empty string clears preedit)
/// * `cursor` - Cursor position within the preedit text (byte offset), or None for end
pub fn simulate_ime_preedit<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    preedit: &str,
    cursor: Option<usize>,
) {
    i_slint_core::tests::simulate_ime_preedit(
        preedit,
        cursor,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate IME commit (finalize composition).
///
/// This simulates the behavior of an IME committing text, replacing any active preedit
/// with the final text.
///
/// # Arguments
/// * `component` - The component containing the focused TextInput
/// * `text` - The text to commit
/// * `cursor_offset` - Where to place cursor relative to inserted text end
///   (0 = at end, negative = before, positive = after)
pub fn simulate_ime_commit<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    text: &str,
    cursor_offset: i32,
) {
    i_slint_core::tests::simulate_ime_commit(
        text,
        cursor_offset,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate setting a composing region on existing text.
///
/// The composing region marks a range of existing committed text as "being edited" by the IME.
/// This is used by autocorrect features.
///
/// # Arguments
/// * `component` - The component containing the focused TextInput
/// * `region` - The (start, end) byte offsets, or None to clear the region
pub fn simulate_ime_set_composing_region<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    region: Option<(usize, usize)>,
) {
    i_slint_core::tests::simulate_ime_set_composing_region(
        region,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate setting the soft keyboard state.
///
/// This simulates the behavior of a platform reporting soft keyboard visibility changes.
/// It updates the window's virtual keyboard properties which can affect layout.
///
/// # Arguments
/// * `component` - The component whose window to update
/// * `visible` - Whether the keyboard is visible
/// * `height` - Height of the keyboard in logical pixels
pub fn simulate_set_soft_keyboard_state<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    visible: bool,
    height: f32,
) {
    i_slint_core::tests::simulate_set_soft_keyboard_state(
        visible,
        height,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Get the current soft keyboard state.
///
/// Returns (visible, height) tuple.
pub fn get_soft_keyboard_state<
    X: vtable::HasStaticVTable<i_slint_core::item_tree::ItemTreeVTable>,
    Component: Into<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
) -> (bool, f32) {
    i_slint_core::tests::get_soft_keyboard_state(
        &WindowInner::from_pub(component.window()).window_adapter(),
    )
}

pub fn access_testing_window<R>(
    window: &i_slint_core::api::Window,
    callback: impl FnOnce(&TestingWindow) -> R,
) -> R {
    i_slint_core::window::WindowInner::from_pub(window)
        .window_adapter()
        .internal(i_slint_core::InternalToken)
        .and_then(|wa| (wa as &dyn core::any::Any).downcast_ref::<TestingWindow>())
        .map(callback)
        .expect("access_testing_window called without testing backend/adapter")
}
