// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The desktop example: a Slint window shows the frames the safety UI renders
//! and forwards touch input to it. The UI itself runs on a worker thread
//! through [`slint_safeui_app::app_main`].

mod platform;

use std::sync::mpsc::Sender;
use std::thread::Thread;

use platform::{DesktopPlatform, HEIGHT, Input, WIDTH};

slint::slint! {
    export component SimWindow inherits Window {
        in property <image> image <=> screen.source;
        preferred-width: 320px;
        preferred-height: 240px;

        screen := Image {
            width: 100%;
            height: 100%;
        }

        touch := TouchArea {
            width: 100%;
            height: 100%;
            pointer-event(event) => {
                if (event.kind == PointerEventKind.down) {
                    root.pressed(touch.mouse-x / 1px, touch.mouse-y / 1px);
                } else if (event.kind == PointerEventKind.up) {
                    root.released(touch.mouse-x / 1px, touch.mouse-y / 1px);
                }
            }
        }

        callback pressed(/* x */ float, /* y */ float);
        callback released(/* x */ float, /* y */ float);
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

fn main() {
    let (pixel_sender, pixel_receiver) = smol::channel::unbounded::<Vec<u8>>();
    let (input_sender, input_receiver) = std::sync::mpsc::channel::<Input>();

    let worker = std::thread::spawn(move || {
        let platform = DesktopPlatform::new(pixel_sender, input_receiver);
        slint_safeui_app::block_on(slint_safeui_app::app_main(platform));
    });
    let worker_thread = worker.thread().clone();

    let window = SimWindow::new().unwrap();

    let send = notifier(&input_sender, &worker_thread);
    window.on_pressed(move |x, y| send(Input::Pressed { x, y }));
    let send = notifier(&input_sender, &worker_thread);
    window.on_released(move |x, y| send(Input::Released { x, y }));

    let weak = window.as_weak();
    slint::spawn_local(async move {
        while let Ok(pixels) = pixel_receiver.recv().await {
            if let Some(window) = weak.upgrade() {
                let buffer = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                    &pixels, WIDTH, HEIGHT,
                );
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
