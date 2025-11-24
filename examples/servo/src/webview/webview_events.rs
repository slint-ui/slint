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

use crate::{MyApp, WebviewLogic};

use super::adapter::{SlintServoAdapter, upgrade_adapter};
use super::key_event_util::convert_slint_key_event_to_servo_keyboard_event;

pub struct WebViewEvents<'a> {
    app: &'a MyApp,
    adapter: Rc<SlintServoAdapter>,
}

impl<'a> WebViewEvents<'a> {
    pub fn new(app: &'a MyApp, adapter: Rc<SlintServoAdapter>) {
        let instance = Self { app, adapter };
        instance.on_url();
        instance.on_theme();
        instance.on_resize();
        instance.on_scroll();
        instance.on_buttons();
        instance.on_pointer();
        instance.on_key_event();
    }

    fn on_url(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_loadUrl(move |url| {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            let url = Url::parse(url.as_str()).expect("Failed to parse url");
            webview.load(url);
        });
    }

    fn on_theme(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_theme(move |color_scheme| {
            let theme = if color_scheme == ColorScheme::Dark { Theme::Dark } else { Theme::Light };
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            // Theme not updating until mouse move over it
            // https://github.com/servo/servo/issues/40268
            webview.notify_theme_change(theme);
        });
    }

    fn on_resize(&self) {
        let app_weak = self.app.as_weak();
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_resize(move |width, height| {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();

            let scale_factor =
                app_weak.upgrade().expect("Failed to upgrade app").window().scale_factor();
            let scale = Scale::new(scale_factor);

            webview.set_hidpi_scale_factor(scale);

            let size = Size2D::new(width, height);
            let physical_size = PhysicalSize::new(size.width as u32, size.height as u32);
            let rect: Box2D<f32, DevicePixel> =
                Box2D::from_origin_and_size(Point2D::origin(), size);

            webview.move_resize(rect);
            webview.resize(physical_size);
        });
    }

    fn on_scroll(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_scroll(
            move |initial_x, initial_y, delta_x, delta_y| {
                let adapter = upgrade_adapter(&adapter_weak);
                let webview = adapter.webview();

                let point = DevicePoint::new(initial_x, initial_y);
                let moved_by = DeviceVector2D::new(delta_x, delta_y);
                // Invert delta to match Servo's coordinate system
                let servo_delta = -moved_by;

                webview.notify_scroll_event(Scroll::Delta(servo_delta.into()), point.into());
            },
        );
    }

    fn on_buttons(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_back(move || {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            webview.go_back(1);
        });

        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_forward(move || {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            webview.go_forward(1);
        });

        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_reload(move || {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            webview.reload();
        });
    }

    fn on_pointer(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_pointer(move |pointer_event, x, y| {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            let point = DevicePoint::new(x, y);
            let input_event = Self::convert_slint_pointer_event_to_servo_input_event(
                &pointer_event,
                point.into(),
            );
            webview.notify_input_event(input_event);
        });
    }

    fn on_key_event(&self) {
        let adapter_weak = Rc::downgrade(&self.adapter);
        self.app.global::<WebviewLogic>().on_key_event(move |event, is_pressed| {
            let adapter = upgrade_adapter(&adapter_weak);
            let webview = adapter.webview();
            let keybord_event = convert_slint_key_event_to_servo_keyboard_event(&event, is_pressed);
            let input_event = InputEvent::Keyboard(keybord_event);
            webview.notify_input_event(input_event);
        });
    }

    fn convert_slint_pointer_event_to_servo_input_event(
        pointer_event: &PointerEvent,
        point: WebViewPoint,
    ) -> InputEvent {
        if pointer_event.is_touch {
            Self::handle_touch_events(pointer_event, point)
        } else {
            Self::handle_mouse_events(pointer_event, point)
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
        let button = Self::get_mouse_button(pointer_event);
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
}
