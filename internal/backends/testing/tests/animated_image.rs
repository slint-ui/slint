// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "renderer-software")]

use slint::ComponentHandle;

slint::slint! {
    #[edition = "1.19"]
    export component App inherits Window {
        width: 16px;
        height: 8px;
        in property <bool> shown: true;
        Image {
            x: 0;
            visible: root.shown;
            width: 8px;
            height: 8px;
            // Four frames: red for 100ms, green for 200ms, blue for 300ms, and white for 400ms.
            // The file plays twice.
            source: @image-url("../../../core/graphics/image/testdata/twice.png");
        }
        Image {
            x: 8px;
            visible: root.shown;
            width: 8px;
            height: 8px;
            source: @image-url("../../../core/graphics/image/testdata/twice.png", nine-slice(1));
        }
    }
}

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

fn pixel(app: &App) -> [u8; 3] {
    let snapshot = app.window().take_snapshot().unwrap();
    let pixel = |x: usize| {
        let p = snapshot.as_slice()[4 * snapshot.width() as usize + x];
        [p.r, p.g, p.b]
    };
    // The nine-slice image plays like the first one.
    assert_eq!(pixel(12), pixel(4));
    pixel(4)
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

    assert_eq!(pixel(&app), RED);
    advance(100);
    assert_eq!(pixel(&app), GREEN);
    advance(200);
    assert_eq!(pixel(&app), BLUE);
    advance(299);
    assert_eq!(pixel(&app), BLUE);
    advance(1);
    assert_eq!(pixel(&app), WHITE);

    // A single step past several frames shows the frame for the elapsed time.
    advance(500);
    assert_eq!(pixel(&app), GREEN);

    // The animation keeps time while the item isn't drawn.
    app.set_shown(false);
    advance(200);
    app.set_shown(true);
    assert_eq!(pixel(&app), BLUE);

    // Without a redraw, no timer stays scheduled.
    assert!(slint::platform::duration_until_next_timer_update().is_some());
    advance(5000);
    assert_eq!(slint::platform::duration_until_next_timer_update(), None);

    // The animation ends on the last frame of the second play.
    assert_eq!(pixel(&app), WHITE);
    assert_eq!(slint::platform::duration_until_next_timer_update(), None);
}
