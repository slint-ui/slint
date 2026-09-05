// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// A size set before the window is shown is kept in both dimensions once it is shown

fn main() {
    slint::slint! {
        export component App inherits Window {
            preferred-width: 400px;
            preferred-height: 300px;
            Text { text: "set_size before show"; }
        }
    }
    let app = App::new().unwrap();
    let requested = slint::LogicalSize::new(700., 500.);
    app.window().set_size(slint::WindowSize::Logical(requested));
    app.show().unwrap();
    // The size is right until the first frame, which resizes the window to the item's size
    slint::Timer::single_shot(std::time::Duration::from_millis(1000), move || {
        assert_eq!(app.window().size(), requested.to_physical(app.window().scale_factor()));
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop().unwrap();
}
