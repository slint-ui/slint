// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use euclid::{Box2D, Point2D, Size2D, Vector2D};

use i_slint_core::items::{PointerEvent, PointerEventKind};
use servo::{
    InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, TouchEvent,
    TouchEventType, TouchId,
    webrender_api::{
        ScrollLocation,
        units::{DevicePixel, DevicePoint},
    },
};
use slint::{ComponentHandle, platform::PointerEventButton};
use url::Url;
use winit::dpi::PhysicalSize;

use crate::{
    WebviewLogic,
    adapter::{SlintServoAdapter, upgrade_adapter},
};

pub fn on_app_callbacks(adapter: Rc<SlintServoAdapter>) {
    on_resize(adapter.clone());
    on_buttons(adapter.clone());
    on_scroll(adapter.clone());
    on_pointer(adapter.clone());
}

fn on_buttons(adapter: Rc<SlintServoAdapter>) {
    let app = adapter.app();

    let adapter_weak = Rc::downgrade(&adapter);
    app.on_back(move || {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        webview.go_back(1);
    });

    let adapter_weak = Rc::downgrade(&adapter);
    app.on_forward(move || {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        webview.go_forward(1);
    });

    let adapter_weak = Rc::downgrade(&adapter);
    app.on_reload(move || {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        webview.reload();
    });

    let adpater_weak = Rc::downgrade(&adapter);
    app.on_go(move |url| {
        let adapter = upgrade_adapter(&adpater_weak);
        let webview = adapter.webview();
        let url = Url::parse(url.as_str()).expect("Failed to parse url");
        webview.load(url);
    });
}

fn on_resize(adapter: Rc<SlintServoAdapter>) {
    let app = adapter.app();

    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>()
        .on_resize(move |width, height| {
            let adapter = upgrade_adapter(&adapter_weak);

            let webview = adapter.webview();

            let scale_factor = adapter.scale_factor();

            let size = Size2D::new(width, height) * scale_factor;

            let physical_size = PhysicalSize::new(size.width as u32, size.height as u32);

            let rect: Box2D<f32, DevicePixel> =
                Box2D::from_origin_and_size(Point2D::origin(), size);

            webview.move_resize(rect);
            webview.resize(physical_size);
        });
}

fn on_scroll(adapter: Rc<SlintServoAdapter>) {
    let app = adapter.app();

    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>()
        .on_scroll(move |initial_x, initial_y, delta_x, delta_y| {
            let adapter = upgrade_adapter(&adapter_weak);

            println!(
                "Scroll event initial_x:{} initial_y:{} delta_x:{} delta_y:{}",
                initial_x, initial_y, delta_x, delta_y
            );

            let webview = adapter.webview();

            let scale_factor = adapter.scale_factor();

            let point = DevicePoint::new(initial_x * scale_factor, initial_y * scale_factor);

            let moved_by = Vector2D::new(delta_x, delta_y);
            let servo_delta = -moved_by;

            webview.notify_scroll_event(ScrollLocation::Delta(servo_delta), point.to_i32());
        });
}

fn on_pointer(adapter: Rc<SlintServoAdapter>) {
    let app = adapter.app();

    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>()
        .on_pointer(move |pointer_event, x, y| {
            let adapter = upgrade_adapter(&adapter_weak);

            let webview = adapter.webview();

            let scale_factor = adapter.scale_factor();

            let point = DevicePoint::new(x * scale_factor, y * scale_factor);

            let input_event =
                convert_slint_pointer_event_to_servo_input_event(&pointer_event, point);

            webview.notify_input_event(input_event);
        });
}

fn convert_slint_pointer_event_to_servo_input_event(
    pointer_event: &PointerEvent,
    point: DevicePoint,
) -> InputEvent {
    if pointer_event.is_touch {
        handle_touch_events(pointer_event, point)
    } else {
        _handle_mouse_events(pointer_event, point)
    }
}

fn handle_touch_events(pointer_event: &PointerEvent, point: DevicePoint) -> InputEvent {
    let touch_id = TouchId(1);
    let touch_event = match pointer_event.kind {
        PointerEventKind::Down => TouchEvent::new(TouchEventType::Down, touch_id, point),
        PointerEventKind::Up => TouchEvent::new(TouchEventType::Up, touch_id, point),
        _ => TouchEvent::new(TouchEventType::Move, touch_id, point),
    };
    InputEvent::Touch(touch_event)
}

fn _handle_mouse_events(pointer_event: &PointerEvent, point: DevicePoint) -> InputEvent {
    let button = _get_mouse_button(pointer_event);
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

fn _get_mouse_button(point_event: &PointerEvent) -> MouseButton {
    match point_event.button {
        PointerEventButton::Left => MouseButton::Left,
        PointerEventButton::Right => MouseButton::Right,
        PointerEventButton::Middle => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}
