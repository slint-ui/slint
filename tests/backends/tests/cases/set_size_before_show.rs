// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The window size after show(): a size set before the window is shown is kept in both dimensions,
// and without one the window gets the component's preferred size.
//
// Only Wayland exercises the regression of a lost width on winit: the other platforms render the
// first frame before mapping the window, and that frame's resize event restores the width.

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

/// Runs the event loop past the first frame, which resizes the window to the item's size, then
/// checks the window against an expected logical size, and that the component sees the same.
fn check_after_show(
    measure: impl Fn() -> (slint::LogicalSize, f32, f32) + 'static,
    expected: slint::LogicalSize,
) {
    slint::Timer::single_shot(std::time::Duration::from_millis(1000), move || {
        // Compare in logical pixels with a tolerance: the physical size is rounded by the
        // windowing system, and the conversion back is not exact on fractional scale factors.
        let close = |a: f32, b: f32| (a - b).abs() < 1.;
        let (size, width, height) = measure();
        assert!(
            close(size.width, expected.width) && close(size.height, expected.height),
            "window size {size:?} != {expected:?}"
        );
        assert!(
            close(width, expected.width) && close(height, expected.height),
            "component size {width}x{height} != {expected:?}"
        );
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop().unwrap();
}

/// The window's logical size, and the size the component reports
macro_rules! measure {
    ($app:expr) => {{
        let weak = $app.as_weak();
        move || {
            let app = weak.unwrap();
            let window = app.window();
            (
                window.size().to_logical(window.scale_factor()),
                app.get_reported_width(),
                app.get_reported_height(),
            )
        }
    }};
}

#[satchel::test]
fn logical_size_before_show() {
    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(LOGICAL));
    app.show().unwrap();
    check_after_show(measure!(app), LOGICAL);
}

#[satchel::test]
fn physical_size_before_show() {
    let physical = slint::PhysicalSize::new(650, 450);
    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Physical(physical));
    app.show().unwrap();
    let expected = physical.to_logical(app.window().scale_factor());
    check_after_show(measure!(app), expected);
}

#[satchel::test]
fn preferred_size() {
    let app = Fixed::new().unwrap();
    app.show().unwrap();
    check_after_show(measure!(app), slint::LogicalSize::new(400., 300.));
}

#[satchel::test]
fn preferred_height_from_the_preferred_width() {
    let app = Dependent::new().unwrap();
    app.show().unwrap();
    check_after_show(measure!(app), slint::LogicalSize::new(400., 200.));
}

#[satchel::test]
fn size_before_show_with_a_dependent_height() {
    let app = Dependent::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(LOGICAL));
    app.show().unwrap();
    check_after_show(measure!(app), LOGICAL);
}

#[satchel::test]
fn size_after_show() {
    let app = Fixed::new().unwrap();
    app.show().unwrap();
    let weak = app.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
        weak.unwrap().window().set_size(slint::WindowSize::Logical(LOGICAL));
    });
    check_after_show(measure!(app), LOGICAL);
}
