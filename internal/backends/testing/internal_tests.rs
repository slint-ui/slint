// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains helper functions that are used for our internal tests within Slint

use crate::TestingWindow;
use i_slint_core::SharedString;
use i_slint_core::api::ComponentHandle;
pub use i_slint_core::input::TouchPhase;
use i_slint_core::item_tree::ItemTreeVTable;
use i_slint_core::platform::WindowEvent;
pub use i_slint_core::tests::slint_get_mocked_time as get_mocked_time;
pub use i_slint_core::tests::slint_mock_elapsed_time as mock_elapsed_time;
use i_slint_core::window::WindowInner;

/// Simulate a mouse click at `(x, y)` and release after a while at the same position
pub fn send_mouse_click<
    X: vtable::HasStaticVTable<ItemTreeVTable> + 'static,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
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

/// Simulate entering a keyboard shortcut or other "nested" character sequence
pub fn send_keyboard_shortcut<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    keys: impl IntoIterator<Item = impl Into<char>>,
) {
    let keys: Vec<_> = keys.into_iter().map(Into::into).collect();
    for key in &keys {
        send_keyboard_char(component, key.clone(), true);
    }
    for key in keys.iter().rev() {
        send_keyboard_char(component, key.clone(), false);
    }
}

/// Simulate entering a sequence of ascii characters key by (pressed or released).
pub fn send_keyboard_char<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
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
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
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
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    factor: f32,
) {
    component.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: factor });
}

/// Send a platform pinch gesture event to the component's window.
///
/// `delta` is the incremental scale change (e.g. 0.0 for start, 0.5 for 50% increase).
/// The PinchGestureHandler accumulates deltas: `scale *= (1.0 + delta)`.
pub fn send_pinch_gesture<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    delta: f32,
    center_x: f32,
    center_y: f32,
    phase: i_slint_core::input::TouchPhase,
) {
    let inner = WindowInner::from_pub(component.window());
    inner.process_mouse_input(i_slint_core::input::MouseEvent::PinchGesture {
        position: i_slint_core::lengths::logical_point_from_api(
            i_slint_core::api::LogicalPosition::new(center_x, center_y),
        ),
        delta,
        phase,
    });
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

/// Runs a future to completion by polling the future and updating the mock time until the future is ready
pub fn block_on<R>(future: impl Future<Output = R>) -> R {
    let mut pinned = core::pin::pin!(future);
    let mut ctx = core::task::Context::from_waker(core::task::Waker::noop());
    loop {
        if let core::task::Poll::Ready(r) = pinned.as_mut().poll(&mut ctx) {
            return r;
        }
        let duration = i_slint_core::platform::duration_until_next_timer_update()
            .unwrap_or(core::time::Duration::from_secs(1));
        mock_elapsed_time(duration.as_millis() as u64);
    }
}
