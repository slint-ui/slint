// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;

slint::slint! {
    export component Comparison inherits Window {
        title: "UIKit over Slint";
        background: #f4f6fa;
        in property <length> list-height: root.height - 148px;
        out property <float> scroll-offset: -list.content-y / 1px;
        out property <float> viewport-x: list.x / 1px;
        out property <float> viewport-y: list.y / 1px;
        out property <float> viewport-width: list.width / 1px;
        out property <float> viewport-height: list.height / 1px;
        out property <float> content-width: list.content-width / 1px;
        out property <float> content-height: list.content-height / 1px;
        callback set-scroll-offset(float);
        callback scroll-to-offset(float);
        set-scroll-offset(offset) => { list.content-y = -offset * 1px; }
        scroll-to-offset(offset) => { list.scroll-to({ x: 0px, y: offset * 1px }, ScrollMode.smooth); }
        list := Flickable {
            x: 8px; y: 102px;
            width: root.width - 16px; height: root.list-height;
            content-width: self.width; content-height: 1000 * 72px;
            for row in 1000 : Rectangle {
                y: row * 72px; height: 72px; width: list.content-width;
                background: mod(row, 2) == 0 ? #e4ebf5 : #ffffff;
                Text {
                    x: 12px; height: parent.height; vertical-alignment: center;
                    text: "Slint " + (row + 1); font-size: 16px; color: #145aaa;
                }
            }
        }
    }
}

thread_local! {
    static APP: std::cell::RefCell<Option<slint::Weak<Comparison>>> = const { std::cell::RefCell::new(None) };
}

fn with_app<R>(default: R, f: impl FnOnce(Comparison) -> R) -> R {
    APP.with(|slot| slot.borrow().as_ref().and_then(slint::Weak::upgrade).map_or(default, f))
}

#[repr(C)]
#[derive(Default)]
struct ScrollGeometry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    content_width: f32,
    content_height: f32,
}

#[unsafe(no_mangle)]
extern "C" fn slint_scroll_geometry() -> ScrollGeometry {
    with_app(ScrollGeometry::default(), |app| ScrollGeometry {
        x: app.get_viewport_x(),
        y: app.get_viewport_y(),
        width: app.get_viewport_width(),
        height: app.get_viewport_height(),
        content_width: app.get_content_width(),
        content_height: app.get_content_height(),
    })
}

#[unsafe(no_mangle)]
extern "C" fn slint_scroll_offset() -> f32 {
    with_app(0.0, |app| app.get_scroll_offset())
}

/// How far Slint's animation clock is behind the platform clock, in milliseconds.
#[unsafe(no_mangle)]
extern "C" fn slint_animation_clock_lag_ms() -> f32 {
    with_app(0.0, |app| {
        let ctx = i_slint_core::window::WindowInner::from_pub(app.window()).context();
        let now = i_slint_core::animations::Instant::now(ctx).as_nanos();
        let tick = i_slint_core::animations::current_tick().as_nanos();
        (now as f64 - tick as f64) as f32 / 1_000_000.
    })
}

#[unsafe(no_mangle)]
extern "C" fn set_slint_scroll_offset(offset: f32) {
    with_app((), |app| app.invoke_set_scroll_offset(offset));
}

#[unsafe(no_mangle)]
extern "C" fn slint_scroll_to(offset: f32) {
    with_app((), |app| app.invoke_scroll_to_offset(offset));
}

unsafe extern "C" {
    fn install_native_scroll(host: *mut std::ffi::c_void);
    fn register_scroll_scene_delegate();
}

fn main() {
    unsafe { register_scroll_scene_delegate() };
    let app = Comparison::new().unwrap();
    if let Some(height) = std::env::var("VIEWPORT_HEIGHT").ok().and_then(|h| h.parse().ok()) {
        app.set_list_height(height);
    }
    APP.with(|slot| *slot.borrow_mut() = Some(app.as_weak()));
    let weak = app.as_weak();
    slint::spawn_local(async move {
        let app = weak.unwrap();
        let window = app.window().winit_window().await.unwrap();
        let RawWindowHandle::UiKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("This comparison requires iOS");
        };
        unsafe { install_native_scroll(handle.ui_view.as_ptr()) };
    })
    .unwrap();
    app.run().unwrap();
}
