// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use winit::dpi::PhysicalSize;

use euclid::{Box2D, Point2D, Scale, Size2D};

use i_slint_core::items::ColorScheme;
use slint::ComponentHandle;

use servo::{
    InputEvent, Scroll, Theme,
    webrender_api::units::{DevicePixel, DevicePoint, DeviceVector2D},
};

use crate::{MyApp, WebviewLogic};

use super::adapter::{SlintServoAdapter, upgrade_adapter};
use super::key_event_util::convert_slint_key_event_to_servo_keyboard_event;
use super::pointer_event_util::convert_slint_pointer_event_to_servo_input_event;
use super::url_event_util::convert_input_string_to_servo_url;

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
            let url = convert_input_string_to_servo_url(&url);
            webview.load(url.into_url());
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
            let input_event =
                convert_slint_pointer_event_to_servo_input_event(&pointer_event, point.into());
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
}
