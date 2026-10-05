// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "renderer-software")]

use slint::ComponentHandle;

slint::slint! {
    export component App inherits Window {
        width: 16px;
        height: 8px;
        in property <bool> running: true;
        Image {
            x: 0;
            width: 8px;
            height: 8px;
            // Four frames: red for 100ms, green for 200ms, blue for 300ms, and white for 400ms.
            // The file plays twice.
            source: @image-url("../../../core/graphics/image/testdata/twice.png");
            running: root.running;
        }
        Image {
            x: 8px;
            width: 8px;
            height: 8px;
            source: @image-url("../../../core/graphics/image/testdata/twice.png");
            running: false;
        }
    }
}

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

fn pixels(app: &App) -> ([u8; 3], [u8; 3]) {
    let snapshot = app.window().take_snapshot().unwrap();
    let pixel = |x: usize| {
        let p = snapshot.as_slice()[4 * snapshot.width() as usize + x];
        [p.r, p.g, p.b]
    };
    (pixel(4), pixel(12))
}

fn advance(ms: u64) {
    i_slint_backend_testing::testing_backend::mock_elapsed_time(ms);
}

#[test]
fn animated_image_playback() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("software".into()),
        },
    )))
    .unwrap();

    let app = App::new().unwrap();
    app.show().unwrap();

    assert_eq!(pixels(&app), (RED, RED));
    advance(100);
    assert_eq!(pixels(&app), (GREEN, RED));
    advance(200);
    assert_eq!(pixels(&app), (BLUE, RED));

    // Pausing keeps the current frame and position.
    advance(100);
    app.set_running(false);
    assert_eq!(pixels(&app), (BLUE, RED));
    advance(1000);
    assert_eq!(pixels(&app), (BLUE, RED));
    app.set_running(true);
    assert_eq!(pixels(&app), (BLUE, RED));
    advance(199);
    assert_eq!(pixels(&app), (BLUE, RED));
    advance(1);
    assert_eq!(pixels(&app), (WHITE, RED));

    // A single step past several frames shows the frame for the elapsed time.
    advance(500);
    assert_eq!(pixels(&app), (GREEN, RED));

    // Without a redraw, no timer stays scheduled.
    assert!(slint::platform::duration_until_next_timer_update().is_some());
    advance(5000);
    assert_eq!(slint::platform::duration_until_next_timer_update(), None);

    // The animation ends on the last frame of the second play.
    assert_eq!(pixels(&app), (WHITE, RED));
    assert_eq!(slint::platform::duration_until_next_timer_update(), None);

    // Resuming a finished animation keeps the last frame.
    app.set_running(false);
    assert_eq!(pixels(&app), (WHITE, RED));
    app.set_running(true);
    assert_eq!(pixels(&app), (WHITE, RED));
    assert_eq!(slint::platform::duration_until_next_timer_update(), None);
}
