// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::{
    platform::PointerEventButton,
    private_unstable_api::re_exports::{PointerEvent, PointerEventKind},
};

use servo::{
    InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, TouchEvent,
    TouchEventType, TouchId, WebViewPoint,
};

pub fn convert_slint_pointer_event_to_servo_input_event(
    pointer_event: &PointerEvent,
    point: WebViewPoint,
) -> InputEvent {
    if pointer_event.is_touch {
        handle_touch_events(pointer_event, point)
    } else {
        handle_mouse_events(pointer_event, point)
    }
}

fn handle_touch_events(pointer_event: &PointerEvent, point: WebViewPoint) -> InputEvent {
    let touch_id = TouchId(1);
    let touch_event = match pointer_event.kind {
        PointerEventKind::Down => TouchEvent::new(TouchEventType::Down, touch_id, point),
        PointerEventKind::Up => TouchEvent::new(TouchEventType::Up, touch_id, point),
        _ => TouchEvent::new(TouchEventType::Move, touch_id, point),
    };
    InputEvent::Touch(touch_event)
}

fn handle_mouse_events(pointer_event: &PointerEvent, point: WebViewPoint) -> InputEvent {
    let button = get_mouse_button(pointer_event);
    match pointer_event.kind {
        PointerEventKind::Down => {
            let mouse_event = MouseButtonEvent::new(MouseButtonAction::Down, button, point);
            InputEvent::MouseButton(mouse_event)
        }
        PointerEventKind::Up => {
            let mouse_event = MouseButtonEvent::new(MouseButtonAction::Up, button, point);
            InputEvent::MouseButton(mouse_event)
        }
        _ => InputEvent::MouseMove(MouseMoveEvent::new(point)),
    }
}

fn get_mouse_button(point_event: &PointerEvent) -> MouseButton {
    match point_event.button {
        PointerEventButton::Left => MouseButton::Left,
        PointerEventButton::Right => MouseButton::Right,
        PointerEventButton::Middle => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}
