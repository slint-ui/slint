// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cspell:ignore callgrind atstart relayout

//! Instruction-count benchmark for the font lookups behind `Text::layout_info`.
//!
//! Run under callgrind, toggling collection on one phase at a time:
//!
//! ```sh
//! cargo test -p i-slint-core --test font_query_bench --release --no-run
//! valgrind --tool=callgrind --collect-atstart=no --toggle-collect='*phase_relayout*' \
//!     <test binary> --test-threads=1
//! ```
//!
//! The scene is 500 `Text` items in a `VerticalLayout` with a handful of distinct font
//! requests, mostly `no-wrap`, as in a list of chat messages.

#![cfg(all(feature = "std", feature = "shared-parley"))]

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, SoftwareRenderer, TargetPixel,
};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{Model, SharedString};
use std::rc::Rc;

const ITEM_COUNT: usize = 500;
const WIDTH: usize = 400;
const HEIGHT: usize = 8;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
}

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|w| w.clone()))
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
struct Pixel(bool);

impl TargetPixel for Pixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }
    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}

slint::slint! {
    export component Bench inherits Window {
        in property <[string]> model;
        in property <length> gap: 0px;
        VerticalLayout {
            spacing: gap;
            for s[i] in model : Text {
                text: s;
                font-size: mod(i, 3) == 0 ? 12px : mod(i, 3) == 1 ? 14px : 18px;
                font-weight: mod(i, 2) == 0 ? 400 : 700;
                font-family: mod(i, 4) == 0 ? "Noto Sans" : "";
                wrap: mod(i, 5) == 0 ? word-wrap : no-wrap;
                overflow: mod(i, 7) == 0 ? elide : clip;
            }
        }
    }
}

fn render(renderer: &SoftwareRenderer) {
    let mut buf = vec![Pixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
}

fn message(i: usize, suffix: &str) -> SharedString {
    let words = ["lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing"];
    let mut s = format!("Message {i}:");
    for w in words.iter().cycle().skip(i % 7).take(2 + i % 9) {
        s.push(' ');
        s.push_str(w);
    }
    s.push_str(suffix);
    s.into()
}

#[inline(never)]
fn phase_first_frame(window: &MinimalSoftwareWindow) {
    assert!(window.draw_if_needed(render));
}

/// Three layout passes over unchanged text: every `Text` answers `layout_info` again from
/// its cached shaping, so only the per-call font lookups remain.
#[inline(never)]
fn phase_relayout(ui: &Bench, window: &MinimalSoftwareWindow) {
    for gap in 1..=3 {
        ui.set_gap(gap as f32);
        assert!(window.draw_if_needed(render));
    }
}

/// Every text changes, so every item is shaped again.
#[inline(never)]
fn phase_retext(model: &slint::VecModel<SharedString>, window: &MinimalSoftwareWindow) {
    for i in 0..ITEM_COUNT {
        model.set_row_data(i, message(i, "!"));
    }
    assert!(window.draw_if_needed(render));
}

#[test]
fn font_query_bench() {
    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    i_slint_backend_testing::configure_test_fonts();
    let window = WINDOW.with(|w| w.clone());
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));

    let model =
        Rc::new(slint::VecModel::from((0..ITEM_COUNT).map(|i| message(i, "")).collect::<Vec<_>>()));
    let ui = Bench::new().unwrap();
    ui.set_model(slint::ModelRc::from(model.clone()));
    ui.show().unwrap();

    phase_first_frame(&window);
    phase_relayout(&ui, &window);
    phase_retext(&model, &window);
}
