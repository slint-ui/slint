// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The desktop example: a Slint window shows the frames the safety UI renders
//! and forwards input to it. The UI itself runs on a worker thread through
//! [`slint_safeui_app::app_main`].

mod platform;

use std::sync::mpsc::Sender;
use std::thread::Thread;

use slint::platform::PointerEventButton;

use platform::{DesktopPlatform, HEIGHT, Input, WIDTH};

slint::slint! {
    export component SimWindow inherits Window {
        in property <image> image <=> screen.source;
        preferred-width: 640px;
        preferred-height: 480px;

        // The overlay captures all pointer and keyboard input and forwards it
        // to the safety UI running on the worker thread.
        screen := Image {
            width: 100%;
            height: 100%;
        }

        touch := TouchArea {
            width: 100%;
            height: 100%;
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
                root.pointer-scrolled(
                    touch.mouse-x / 1px, touch.mouse-y / 1px,
                    event.delta-x / 1px, event.delta-y / 1px,
                );
                return accept;
            }
        }

        focus-scope := FocusScope {
            key-pressed(event) => {
                if (event.repeat) {
                    root.key-repeated(event.text);
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
        callback key-repeated(/* text */ string);
        callback key-released(/* text */ string);
    }
}

/// A closure that sends an [`Input`] to the worker and wakes it.
fn notifier(input: &Sender<Input>, worker: &Thread) -> impl Fn(Input) + 'static {
    let input = input.clone();
    let worker = worker.clone();
    move |event| {
        let _ = input.send(event);
        worker.unpark();
    }
}

fn button(is_left: bool) -> PointerEventButton {
    if is_left { PointerEventButton::Left } else { PointerEventButton::Right }
}

fn first_char(text: &slint::SharedString) -> Option<char> {
    text.chars().next()
}

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded::<Vec<slint::Rgb8Pixel>>();
    let (input_sender, input_receiver) = std::sync::mpsc::channel::<Input>();

    let worker = std::thread::spawn(move || {
        let platform = DesktopPlatform::new(pixel_sender, input_receiver);
        slint_safeui_app::block_on(slint_safeui_app::app_main(platform))
            .expect("the UI event loop failed");
    });
    let worker_thread = worker.thread().clone();

    let window = SimWindow::new().unwrap();

    let send = notifier(&input_sender, &worker_thread);
    window.on_pointer_pressed(move |x, y, left| {
        send(Input::PointerPressed { x, y, button: button(left) })
    });
    let send = notifier(&input_sender, &worker_thread);
    window.on_pointer_released(move |x, y, left| {
        send(Input::PointerReleased { x, y, button: button(left) })
    });
    let send = notifier(&input_sender, &worker_thread);
    window.on_pointer_moved(move |x, y| send(Input::PointerMoved { x, y }));
    let send = notifier(&input_sender, &worker_thread);
    window.on_pointer_scrolled(move |x, y, delta_x, delta_y| {
        send(Input::PointerScrolled { x, y, delta_x, delta_y })
    });
    let send = notifier(&input_sender, &worker_thread);
    window.on_key_pressed(move |text| {
        if let Some(c) = first_char(&text) {
            send(Input::KeyPressed(c));
        }
    });
    let send = notifier(&input_sender, &worker_thread);
    window.on_key_repeated(move |text| {
        if let Some(c) = first_char(&text) {
            send(Input::KeyRepeated(c));
        }
    });
    let send = notifier(&input_sender, &worker_thread);
    window.on_key_released(move |text| {
        if let Some(c) = first_char(&text) {
            send(Input::KeyReleased(c));
        }
    });

    let weak = window.as_weak();
    slint::spawn_local(async move {
        while let Ok(pixels) = pixel_receiver.recv().await {
            if let Some(window) = weak.upgrade() {
                let mut buffer = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(WIDTH, HEIGHT);
                buffer.make_mut_slice().copy_from_slice(&pixels);
                window.set_image(slint::Image::from_rgb8(buffer));
            }
        }
    })
    .unwrap();

    window.run().unwrap();

    // The window closed: stop the worker and wait for it.
    let _ = input_sender.send(Input::Quit);
    worker_thread.unpark();
    let _ = worker.join();
}
