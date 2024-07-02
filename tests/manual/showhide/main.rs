// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

slint::slint! {


export component TestWindow inherits Window {
    Text {
        text: "This window will toggle visibility (show/hide)\nevery 5 seconds. When visible, close it via window decorations\nto close the entire app.";
    }
}

}

fn main() -> Result<(), slint::PlatformError> {
    // Pretend that the windowing system has the same behaviour as
    std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1");

    let test_window = TestWindow::new()?;
    test_window.show()?;

    test_window.window().on_close_requested(|| {
        slint::quit_event_loop().unwrap();
        slint::CloseRequestResponse::HideWindow
    });

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(2), move || {
        if test_window.window().is_visible() {
            eprintln!("Hiding");
            test_window.hide().unwrap();
        } else {
            eprintln!("Showing");
            test_window.show().unwrap();
        }
    });

    slint::run_event_loop_until_quit()
}
