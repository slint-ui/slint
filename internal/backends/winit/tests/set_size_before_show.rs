// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The window size after show(): a size set before the window is shown is kept in both dimensions,
// and without one the window gets the component's preferred size.
//
// Only Wayland exercises the regression of a lost width: the other platforms render the first
// frame before mapping the window, and that frame's resize event restores the width.

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

/// Checks the window against an expected logical size, and that the component sees the same.
struct Expectation {
    name: &'static str,
    /// The window's logical size, and the size the component reports
    measure: Box<dyn Fn() -> (slint::LogicalSize, f32, f32)>,
    expected: slint::LogicalSize,
}

impl Expectation {
    fn check(&self) {
        // Compare in logical pixels with a tolerance: the physical size is rounded by the
        // windowing system, and the conversion back is not exact on fractional scale factors.
        let close = |a: f32, b: f32| (a - b).abs() < 1.;
        let (size, width, height) = (self.measure)();
        let expected = self.expected;
        assert!(
            close(size.width, expected.width) && close(size.height, expected.height),
            "{}: window size {size:?} != {expected:?}",
            self.name
        );
        assert!(
            close(width, expected.width) && close(height, expected.height),
            "{}: component size {width}x{height} != {expected:?}",
            self.name
        );
    }
}

macro_rules! expectation {
    ($name:literal, $app:expr, $expected:expr) => {{
        let app = $app;
        Expectation {
            name: $name,
            measure: Box::new(move || {
                let window = app.window();
                (
                    window.size().to_logical(window.scale_factor()),
                    app.get_reported_width(),
                    app.get_reported_height(),
                )
            }),
            expected: $expected,
        }
    }};
}

fn main() {
    slint::BackendSelector::new().backend_name("winit".into()).select().unwrap();

    let logical = slint::LogicalSize::new(700., 500.);
    let physical = slint::PhysicalSize::new(650, 450);
    let mut expectations = Vec::new();

    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(logical));
    app.show().unwrap();
    expectations.push(expectation!("logical size before show", app, logical));

    let app = Fixed::new().unwrap();
    app.window().set_size(slint::WindowSize::Physical(physical));
    app.show().unwrap();
    let scale_factor = app.window().scale_factor();
    expectations.push(expectation!(
        "physical size before show",
        app,
        physical.to_logical(scale_factor)
    ));

    let app = Fixed::new().unwrap();
    app.show().unwrap();
    expectations.push(expectation!("preferred size", app, slint::LogicalSize::new(400., 300.)));

    let app = Dependent::new().unwrap();
    app.show().unwrap();
    expectations.push(expectation!(
        "preferred height from the preferred width",
        app,
        slint::LogicalSize::new(400., 200.)
    ));

    let app = Dependent::new().unwrap();
    app.window().set_size(slint::WindowSize::Logical(logical));
    app.show().unwrap();
    expectations.push(expectation!("size before show with a dependent height", app, logical));

    let app = Fixed::new().unwrap();
    app.show().unwrap();
    let weak = app.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
        weak.unwrap().window().set_size(slint::WindowSize::Logical(logical));
    });
    expectations.push(expectation!("size after show", app, logical));

    // Wait past the first frame, which resizes the window to the item's size
    slint::Timer::single_shot(std::time::Duration::from_millis(1000), move || {
        for expectation in &expectations {
            expectation.check();
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop().unwrap();
}
