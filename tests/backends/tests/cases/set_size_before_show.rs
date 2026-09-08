// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The window size after show(): a size set before the window is shown is kept in both dimensions,
// and without one the window gets the component's preferred size.
//
// Only Wayland exercises the regression of a lost width on winit: the other platforms render the
// first frame before mapping the window, and that frame's resize event restores the width.

use i_slint_backend_winit::WinitWindowAccessor;
use std::cell::Cell;
use std::rc::Rc;

slint::slint! {
    export component Fixed inherits Window {
        preferred-width: 400px;
        preferred-height: 300px;
        out property <length> reported-width: root.width;
        out property <length> reported-height: root.height;
        Text { text: "set_size before show"; }
    }
    // The preferred height depends on the width, as with wrapping text
    export component Dependent inherits Window {
        preferred-width: 400px;
        out property <length> reported-width: root.width;
        out property <length> reported-height: root.height;
        VerticalLayout {
            Rectangle { preferred-height: self.width / 2; }
        }
    }
}

const LOGICAL: slint::LogicalSize = slint::LogicalSize { width: 700., height: 500. };

/// The window once shown: its logical size and scale factor, and the size the component reports.
struct Measured {
    size: slint::LogicalSize,
    scale_factor: f32,
    width: f32,
    height: f32,
}

/// Runs the event loop past the first frame, which resizes the window to the item's size, and
/// returns what `measure` saw then. The checks happen after the loop: a panic inside a timer
/// callback aborts the process on the Qt backend instead of failing the test.
fn run_and_measure(measure: impl Fn() -> Measured + 'static) -> Measured {
    let measured = Rc::new(Cell::new(None));
    let sink = measured.clone();
    slint::Timer::single_shot(std::time::Duration::from_millis(1000), move || {
        sink.set(Some(measure()));
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop().unwrap();
    measured.take().expect("the timer ran before the event loop quit")
}

/// Compares in physical pixels. On a fractional scale factor the preferred size is rounded up
/// to the logical size whose physical size holds it, which overshoots by up to one and a half
/// physical pixels, and the windowing system's own rounding adds up to two more.
fn assert_size(measured: &Measured, expected: slint::LogicalSize) {
    let close = |a: f32, b: f32| (a - b).abs() * measured.scale_factor < 4.;
    assert!(
        close(measured.size.width, expected.width) && close(measured.size.height, expected.height),
        "window size {:?} != {expected:?}",
        measured.size
    );
    assert!(
        close(measured.width, expected.width) && close(measured.height, expected.height),
        "component size {}x{} != {expected:?}",
        measured.width,
        measured.height
    );
}

macro_rules! measure {
    ($app:expr) => {{
        let weak = $app.as_weak();
        move || {
            let app = weak.unwrap();
            let window = app.window();
            let scale_factor = window.scale_factor();
            Measured {
                size: window.size().to_logical(scale_factor),
                scale_factor,
                width: app.get_reported_width(),
                height: app.get_reported_height(),
            }
        }
    }};
}

#[satchel::test]
fn logical_size_before_show() {
    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(LOGICAL));
    app.show().unwrap();
    assert_size(&run_and_measure(measure!(app)), LOGICAL);
}

#[satchel::test]
fn physical_size_before_show() {
    let physical = slint::PhysicalSize::new(650, 450);
    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Physical(physical));
    app.show().unwrap();
    // The scale factor is only known once the window exists, so the expectation comes from the
    // measurement.
    let measured = run_and_measure(measure!(app));
    assert_size(&measured, physical.to_logical(measured.scale_factor));
}

#[satchel::test]
fn preferred_size() {
    let app = Fixed::new().unwrap();
    app.show().unwrap();
    assert_size(&run_and_measure(measure!(app)), slint::LogicalSize::new(400., 300.));
}

#[satchel::test]
fn preferred_height_from_the_preferred_width() {
    let app = Dependent::new().unwrap();
    app.show().unwrap();
    assert_size(&run_and_measure(measure!(app)), slint::LogicalSize::new(400., 200.));
}

#[satchel::test]
fn size_before_show_with_a_dependent_height() {
    let app = Dependent::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(LOGICAL));
    app.show().unwrap();
    assert_size(&run_and_measure(measure!(app)), LOGICAL);
}

#[satchel::test]
fn size_after_show() {
    let app = Fixed::new().unwrap();
    app.show().unwrap();
    let weak = app.as_weak();
    slint::spawn_local(async move {
        let app = weak.unwrap();
        // Set the size once the winit window exists, after the show path has applied the
        // preferred size. Another backend has no winit window and fails this right away; there
        // the window exists from the start.
        let _ = app.window().winit_window().await;
        app.window().set_size(slint::WindowSize::Logical(LOGICAL));
    })
    .unwrap();
    assert_size(&run_and_measure(measure!(app)), LOGICAL);
}
