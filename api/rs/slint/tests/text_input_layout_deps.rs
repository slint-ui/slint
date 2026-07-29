// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A `TextInput`'s size must depend on the text it displays -- the `text` property plus any IME
//! composition -- and on nothing else. If it also depended on the cursor, every blink would dirty
//! the layout and, through it, the shaped-text cache: a full reshape twice a second.
//!
//! This lives in its own test binary because it drives the cursor blinker off a mocked clock,
//! which takes a platform of its own, and a platform can only be installed once per process.

mod common;

use common::TestPixel;
use slint::platform::software_renderer::{MinimalSoftwareWindow, SoftwareRenderer};
use slint::platform::{PlatformError, WindowAdapter};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const WIDTH: usize = 300;
const HEIGHT: usize = 100;
const FLASH_CYCLE: Duration = Duration::from_millis(1000);

thread_local! {
    static NOW: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

struct ClockPlatform;

impl slint::platform::Platform for ClockPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(common::window())
    }

    fn duration_since_start(&self) -> Duration {
        NOW.with(|now| now.get())
    }

    fn cursor_flash_cycle(&self) -> Duration {
        FLASH_CYCLE
    }
}

/// Advances the mocked clock and lets the timers -- the cursor blinker among them -- fire.
fn advance(duration: Duration) {
    NOW.with(|now| now.set(now.get() + duration));
    slint::platform::update_timers_and_animations();
}

/// Renders and reports how often the text had to be reshaped. Panics rather than reporting zero
/// when nothing was drawn, so that a test can't pass by never rendering at all.
fn render_and_get_miss_count(window: &Rc<MinimalSoftwareWindow>) -> u64 {
    let mut miss_count = 0;
    let mut buf = vec![TestPixel::default(); WIDTH * HEIGHT];
    let rendered = window.draw_if_needed(|renderer: &SoftwareRenderer| {
        renderer.text_layout_cache().reset_cache_miss_count();
        renderer.render(buf.as_mut_slice(), WIDTH);
        miss_count = renderer.text_layout_cache().cache_miss_count();
    });
    assert!(rendered, "expected the window to redraw");
    miss_count
}

#[test]
fn blinking_cursor_does_not_reshape() {
    let window = common::setup_with_platform(Box::new(ClockPlatform), WIDTH as u32, HEIGHT as u32);

    slint::slint! {
        export component BlinkComponent inherits Window {
            forward-focus: input;
            in property <string> label: "Hello World, a somewhat longer line of text";
            VerticalLayout {
                input := TextInput {
                    text: label;
                    wrap: word-wrap;
                }
            }
        }
    }

    let ui = BlinkComponent::new().unwrap();
    ui.show().unwrap();
    // Focus the input so that its cursor starts blinking.
    ui.window().dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    render_and_get_miss_count(&window);

    // The first render may well find the text already shaped by the layout pass that preceded it,
    // so provoke a reshape rather than assuming one -- otherwise the blink checks below could pass
    // simply because nothing ever counts as a miss.
    ui.set_label("Goodbye World, a somewhat longer line of text".into());
    window.request_redraw();
    assert!(render_and_get_miss_count(&window) > 0, "expected a cache miss after the text changed");

    for blink in 0..4 {
        advance(FLASH_CYCLE);
        window.request_redraw();
        assert_eq!(render_and_get_miss_count(&window), 0, "blink {blink} reshaped the text");
    }
}
