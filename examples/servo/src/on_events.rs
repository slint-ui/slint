// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use url::Url;
use winit::dpi::PhysicalSize;

use euclid::{Box2D, Point2D, Scale, Size2D};

use i_slint_core::items::{ColorScheme, PointerEvent, PointerEventKind};
use slint::{ComponentHandle, platform::PointerEventButton};

use servo::{
    InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, Scroll, Theme,
    TouchEvent, TouchEventType, TouchId, WebViewPoint,
    webrender_api::units::{DevicePixel, DevicePoint, DeviceVector2D},
};

use crate::{
    MyApp, WebviewLogic,
    adapter::{SlintServoAdapter, upgrade_adapter},
};

pub fn on_app_callbacks(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    on_url(app, adapter.clone());
    on_theme(app, adapter.clone());
    on_resize(app, adapter.clone());
    on_scroll(app, adapter.clone());
    on_buttons(app, adapter.clone());
    on_pointer(app, adapter.clone());
}

fn on_url(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>().on_loadUrl(move |url| {
        let adapter = upgrade_adapter(&adapter_weak);
        let webview = adapter.webview();
        let url = Url::parse(url.as_str()).expect("Failed to parse url");
        webview.load(url);
    });
}

fn on_theme(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>().on_theme(move |color_scheme| {
        let theme = if color_scheme == ColorScheme::Dark { Theme::Dark } else { Theme::Light };

        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        // Theme not updating until mouse move over it
        // https://github.com/servo/servo/issues/40268
        webview.notify_theme_change(theme);
    });
}

// This will always called when slint window show first time and when resize so set scale factor here
fn on_resize(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    let adapter_weak = Rc::downgrade(&adapter);
    let app_weak = app.as_weak();
    app.global::<WebviewLogic>().on_resize(move |width, height| {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        let scale_factor =
            app_weak.upgrade().expect("Failed to upgrade app").window().scale_factor();

        let scale = Scale::new(scale_factor);

        webview.set_hidpi_scale_factor(scale);

        let size = Size2D::new(width, height);

        let physical_size = PhysicalSize::new(size.width as u32, size.height as u32);

        let rect: Box2D<f32, DevicePixel> = Box2D::from_origin_and_size(Point2D::origin(), size);

        webview.move_resize(rect);
        webview.resize(physical_size);
    });
}

fn on_scroll(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>().on_scroll(move |initial_x, initial_y, delta_x, delta_y| {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        let point = DevicePoint::new(initial_x, initial_y);

        let moved_by = DeviceVector2D::new(delta_x, delta_y);

        // Invert delta to match Servo's coordinate system
        let servo_delta = -moved_by;

        webview.notify_scroll_event(Scroll::Delta(servo_delta.into()), point.into());
    });
}

fn on_buttons(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
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
}

fn on_pointer(app: &MyApp, adapter: Rc<SlintServoAdapter>) {
    let adapter_weak = Rc::downgrade(&adapter);
    app.global::<WebviewLogic>().on_pointer(move |pointer_event, x, y| {
        let adapter = upgrade_adapter(&adapter_weak);

        let webview = adapter.webview();

        let point = DevicePoint::new(x, y);

        let input_event =
            convert_slint_pointer_event_to_servo_input_event(&pointer_event, point.into());

        webview.notify_input_event(input_event);
    });
}

/// Converts Slint pointer events to Servo input events.
/// Distinguishes between touch and mouse events for proper handling.
fn convert_slint_pointer_event_to_servo_input_event(
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
