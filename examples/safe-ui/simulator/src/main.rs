// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod desktop_platform;

slint::slint! {
    import { AboutSlint, VerticalBox } from "std-widgets.slint";

    export component MainWindow inherits Window {
        in property <image> image <=> screen.source;

        // Overlay captures all pointer and keyboard input for forwarding
        // to the embedded Slint instance via the FFI event queue.
        screen := Image {
            width: 100%;
            height: 100%;
        }

        touch := TouchArea {
            width: 100%;
            height: 100%;

            clicked => { }

            pointer-event(event) => {
                let x = touch.mouse-x / 1px;
                let y = touch.mouse-y / 1px;

                if (event.kind == PointerEventKind.down) {
                    root.pointer-pressed(x, y, event.button == PointerEventButton.left);
                } else if (event.kind == PointerEventKind.up) {
                    root.pointer-released(x, y, event.button == PointerEventButton.left);
                } else if (event.kind == PointerEventKind.move) {
                    root.pointer-moved(x, y);
                }
            }

            scroll-event(event) => {
                let x = touch.mouse-x / 1px;
                let y = touch.mouse-y / 1px;
                let dx = event.delta-x / 1px;
                let dy = event.delta-y / 1px;
                root.pointer-scrolled(x, y, dx, dy);
                return accept;
            }
        }

        focus-scope := FocusScope {
            key-pressed(event) => {
                if (event.repeat) {
                    root.key-press-repeated(event.text);
                } else {
                    root.key-pressed(event.text);
                }
                return accept;
            }
            key-released(event) => {
                root.key-released(event.text);
                return accept;
            }
        }

        callback pointer-pressed(/* x */ float, /* y */ float, /* is_left */ bool);
        callback pointer-released(/* x */ float, /* y */ float, /* is_left */ bool);
        callback pointer-moved(/* x */ float, /* y */ float);
        callback pointer-scrolled(/* x */ float, /* y */ float, /* dx */ float, /* dy */ float);
        callback key-pressed(/* text */ string);
        callback key-press-repeated(/* text */ string);
        callback key-released(/* text */ string);
    }
}

use slint_safeui_core::ffi_event::FfiEvent;
use slint_safeui_core::ffi_event::FfiPointerButton;

/// Forward an FfiEvent to the embedded Slint instance via the FFI entry point.
fn dispatch(event: FfiEvent) {
    slint_safeui_core::event_dispatch::slint_safeui_dispatch_event(&event as *const _);
}

/// Convert a single-scalar Slint key text to a Unicode code point.
fn text_to_code_point(text: &slint::SharedString) -> Option<u32> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(c as u32)
}

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded();

    desktop_platform::init_channel(pixel_sender);

    let _sim_thread = std::thread::spawn(|| {
        desktop_platform::set_sim_thread(std::thread::current());
        slint_safeui_core::slint_app_main()
    });

    let platform = i_slint_backend_selector::create_backend().unwrap();
    let ctx = i_slint_backend_selector::SlintContext::new(platform);
    let window = MainWindow::new_with_context(ctx.clone()).unwrap();
    let window_weak = window.as_weak();

    window.on_pointer_pressed(move |x, y, is_left| {
        let button = if is_left {
            FfiPointerButton::FfiPointerButton_Left
        } else {
            FfiPointerButton::FfiPointerButton_Right
        };
        dispatch(FfiEvent::pointer_pressed(x as i32, y as i32, button));
    });

    window.on_pointer_released(move |x, y, is_left| {
        let button = if is_left {
            FfiPointerButton::FfiPointerButton_Left
        } else {
            FfiPointerButton::FfiPointerButton_Right
        };
        dispatch(FfiEvent::pointer_released(x as i32, y as i32, button));
    });

    window.on_pointer_moved(move |x, y| {
        dispatch(FfiEvent::pointer_moved(x as i32, y as i32));
    });

    window.on_pointer_scrolled(move |x, y, dx, dy| {
        dispatch(FfiEvent::pointer_scrolled(x as i32, y as i32, dx, dy));
    });

    window.on_key_pressed(move |text| {
        if let Some(code_point) = text_to_code_point(&text) {
            dispatch(FfiEvent::key_pressed(code_point));
        }
    });

    window.on_key_press_repeated(move |text| {
        if let Some(code_point) = text_to_code_point(&text) {
            dispatch(FfiEvent::key_press_repeated(code_point));
        }
    });

    window.on_key_released(move |text| {
        if let Some(code_point) = text_to_code_point(&text) {
            dispatch(FfiEvent::key_released(code_point));
        }
    });

    ctx.spawn_local(async move {
        use desktop_platform::{SCALED_HEIGHT, SCALED_WIDTH};

        loop {
            if let Ok(source_pixels) = pixel_receiver.recv().await
                && let Some(window) = window_weak.upgrade()
            {
                let mut pixel_buf: slint::SharedPixelBuffer<slint::Rgb8Pixel> =
                    slint::SharedPixelBuffer::new(SCALED_WIDTH, SCALED_HEIGHT);
                let pixel_dest = pixel_buf.make_mut_slice();
                pixel_dest.copy_from_slice(&source_pixels);

                window.set_image(slint::Image::from_rgb8(pixel_buf));
            }
        }
    })
    .unwrap();

    window.run().unwrap();
}
