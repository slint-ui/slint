// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore descendents

//! This module contains helper functions that are used for our internal tests within Slint

use crate::TestingWindow;
use i_slint_core::SharedString;
use i_slint_core::api::ComponentHandle;
pub use i_slint_core::input::BackendMouseEvent;
pub use i_slint_core::input::TouchPhase;
use i_slint_core::item_tree::ItemTreeVTable;
pub use i_slint_core::lengths::LogicalPoint;
pub use i_slint_core::platform::InternalEvent;
use i_slint_core::platform::WindowEvent;
pub use i_slint_core::window::PopupWindowLocation;
pub use i_slint_core::window::WindowInner;

/// Simulate a mouse click at `(x, y)` and release after a while at the same position
pub fn send_mouse_click<
    X: vtable::HasStaticVTable<ItemTreeVTable> + 'static,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    x: f32,
    y: f32,
) {
    crate::testing_backend::send_mouse_click(
        x,
        y,
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate entering a sequence of ascii characters key by key.
pub fn send_keyboard_string_sequence<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    sequence: &str,
) {
    crate::testing_backend::send_keyboard_string_sequence(
        &SharedString::from(sequence),
        &WindowInner::from_pub(component.window()).window_adapter(),
    );
}

/// Simulate entering a keyboard shortcut or other "nested" character sequence
pub fn send_key_combo<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    keys: impl IntoIterator<Item = impl Into<char>>,
) {
    let keys: Vec<SharedString> = keys.into_iter().map(|k| k.into().into()).collect();
    send_key_combo_with_text(component, &keys);
}

/// Simulate entering a keyboard shortcut where each key is a text string.
///
/// Unlike [`send_key_combo`], each key can be an arbitrary string,
/// which supports multi-codepoint grapheme clusters (e.g. NFD-encoded Unicode).
pub fn send_key_combo_with_text<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    keys: &[SharedString],
) {
    for key in keys {
        send_keyboard_key_text(component, key, true);
    }
    for key in keys.iter().rev() {
        send_keyboard_key_text(component, key, false);
    }
}

/// Simulate a single key event with the given text (pressed or released).
///
/// Unlike [`send_keyboard_char`], the text is dispatched as a single event,
/// which supports multi-codepoint grapheme clusters.
pub fn send_keyboard_key_text<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    text: &SharedString,
    pressed: bool,
) {
    crate::testing_backend::send_keyboard_key_text(
        text,
        pressed,
        &WindowInner::from_pub(component.window()).window_adapter(),
    )
}

/// Simulate entering a sequence of ascii characters key by (pressed or released).
pub fn send_keyboard_char<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    ch: char,
    pressed: bool,
) {
    send_keyboard_key_text(component, &SharedString::from(ch), pressed)
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

/// Override the locale used for decimal separator detection in the given component's window.
pub fn set_locale<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    locale: &str,
) {
    let inner = WindowInner::from_pub(component.window());
    inner.context().set_locale(locale);
}

/// Send a platform pinch gesture event to the component's window.
///
/// `delta` is the incremental scale change (e.g. 0.0 for start, 0.5 for 50% increase).
/// The ScaleRotateGestureHandler accumulates deltas: `scale *= (1.0 + delta)`.
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
    component.window().dispatch_event(WindowEvent::internal(
        i_slint_core::input::BackendMouseEvent::PinchGesture {
            position: i_slint_core::lengths::logical_point_from_api(
                i_slint_core::api::LogicalPosition::new(center_x, center_y),
            ),
            delta,
            phase,
        },
    ));
}

/// Send a rotation gesture event to the component's window.
///
/// `delta` is the incremental rotation in degrees using the Slint convention:
/// positive = clockwise. The handler accumulates deltas into its `rotation` property.
pub fn send_rotation_gesture<
    X: vtable::HasStaticVTable<ItemTreeVTable>,
    Component: Into<vtable::VRc<ItemTreeVTable, X>> + ComponentHandle,
>(
    component: &Component,
    delta: f32,
    center_x: f32,
    center_y: f32,
    phase: i_slint_core::input::TouchPhase,
) {
    component.window().dispatch_event(WindowEvent::internal(
        i_slint_core::input::BackendMouseEvent::RotationGesture {
            position: i_slint_core::lengths::logical_point_from_api(
                i_slint_core::api::LogicalPosition::new(center_x, center_y),
            ),
            delta,
            phase,
        },
    ));
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
        crate::testing_backend::mock_elapsed_time(duration.as_millis() as u64);
    }
}

/// Returns a textual representation of the accessibility tree of the component's window.
///
/// Each line represents one element that's exposed to assistive technologies, indented by
/// its depth in the accessibility tree: the accessible role, followed by the label in
/// double quotes (if any), all other accessible properties that are set, and the supported
/// accessibility actions. Boolean states appear as bare flags when true (`checkable`,
/// `checked`, `expanded`, ...), a false `enabled` appears as `disabled`, and everything
/// else as `name=value`. Elements with `accessible-role: none` don't appear, but their
/// accessible descendants do, just like in the tree presented to screen readers: the
/// traversal is the same one the AccessKit integration uses.
///
/// Values that are apparent from the structure of the tree are left out to keep the
/// output readable: `item-index` when it matches the element's position among its
/// siblings, `item-count` when it matches the number of accessible children, and the
/// supported actions when they consist of just the default action.
///
/// This is used by the test driver to verify the accessibility tree of an entire user
/// interface as one assertion, for example to ensure that widgets don't expose their
/// internal composition:
///
/// ```text
/// window
///   checkbox "Remember me" checkable
///   button "Log in"
/// ```
pub fn accessibility_tree(component: &impl crate::ElementRoot) -> String {
    use i_slint_core::accessibility::{
        AccessibleStringProperty, SupportedAccessibilityAction, accessible_descendents,
    };
    use std::fmt::Write;

    fn visit(item: &i_slint_core::items::ItemRc, indent: usize, out: &mut String) {
        let children = accessible_descendents(item).collect::<Vec<_>>();
        for (position, child) in children.iter().enumerate() {
            let property =
                |property| child.accessible_string_property(property).filter(|v| !v.is_empty());
            write!(out, "{:indent$}{}", "", child.accessible_role()).unwrap();
            if let Some(label) = property(AccessibleStringProperty::Label) {
                write!(out, " \"{label}\"").unwrap();
            }
            for p in [
                AccessibleStringProperty::Value,
                AccessibleStringProperty::Description,
                AccessibleStringProperty::PlaceholderText,
                AccessibleStringProperty::Id,
            ] {
                if let Some(value) = property(p) {
                    write!(out, " {p}=\"{value}\"").unwrap();
                }
            }
            for p in [
                AccessibleStringProperty::Checkable,
                AccessibleStringProperty::Checked,
                AccessibleStringProperty::Expandable,
                AccessibleStringProperty::Expanded,
                AccessibleStringProperty::ItemSelectable,
                AccessibleStringProperty::ItemSelected,
                AccessibleStringProperty::ReadOnly,
            ] {
                if child.accessible_string_property(p).as_deref() == Some("true") {
                    write!(out, " {p}").unwrap();
                }
            }
            if child.accessible_string_property(AccessibleStringProperty::Enabled).as_deref()
                == Some("false")
            {
                out.push_str(" disabled");
            }
            if let Some(index) = property(AccessibleStringProperty::ItemIndex)
                .filter(|index| index.as_str() != position.to_string())
            {
                write!(out, " item-index={index}").unwrap();
            }
            if let Some(count) = property(AccessibleStringProperty::ItemCount)
                .filter(|count| count.as_str() != accessible_descendents(child).count().to_string())
            {
                write!(out, " item-count={count}").unwrap();
            }
            for p in [
                AccessibleStringProperty::Orientation,
                AccessibleStringProperty::DelegateFocus,
                AccessibleStringProperty::LiveRegion,
                AccessibleStringProperty::ValueMinimum,
                AccessibleStringProperty::ValueMaximum,
                AccessibleStringProperty::ValueStep,
            ] {
                if let Some(value) = property(p) {
                    write!(out, " {p}={value}").unwrap();
                }
            }
            let actions = child.supported_accessibility_actions();
            if actions != SupportedAccessibilityAction::Default && !actions.is_empty() {
                let action_names = [
                    (SupportedAccessibilityAction::Default, "default"),
                    (SupportedAccessibilityAction::Decrement, "decrement"),
                    (SupportedAccessibilityAction::Increment, "increment"),
                    (SupportedAccessibilityAction::Expand, "expand"),
                    (SupportedAccessibilityAction::ReplaceSelectedText, "replace-selected-text"),
                    (SupportedAccessibilityAction::SetValue, "set-value"),
                    (SupportedAccessibilityAction::SetSelection, "set-selection"),
                ]
                .iter()
                .filter(|(action, _)| actions.contains(*action))
                .map(|(_, name)| *name)
                .collect::<Vec<_>>();
                write!(out, " actions={}", action_names.join(",")).unwrap();
            }
            out.push('\n');
            visit(child, indent + 2, out);
        }
    }

    let mut out = String::from("window\n");
    visit(&i_slint_core::items::ItemRc::new_root(component.item_tree()), 2, &mut out);
    out
}
