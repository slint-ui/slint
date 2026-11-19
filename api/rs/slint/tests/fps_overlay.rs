// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod common;

use common::TestPixel;
use slint::{PhysicalPosition, PhysicalSize};

// #10960: with partial rendering, the overlay was only redrawn when the scene under it changed.
#[test]
fn overlay_is_part_of_every_dirty_region() {
    // The renderer reads this when the window is created, below.
    //
    // SAFETY: this is the only test in this binary, so no other thread reads
    // env vars concurrently.
    unsafe {
        std::env::set_var("SLINT_DEBUG_PERFORMANCE", "refresh_lazy,overlay");
    }

    let window = common::setup(200, 200);

    slint::slint! {
        #[edition = "1.20"]
        export component Ui inherits Window {
            in property <color> c: red;
            Rectangle {
                x: 150phx;
                y: 160phx;
                width: 20phx;
                height: 30phx;
                background: c;
            }
        }
    }

    let ui = Ui::new().unwrap();
    ui.show().unwrap();
    let mut buffer = vec![TestPixel(false); 200 * 200];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, 200);
    }));

    ui.set_c(slint::Color::from_rgb_u8(0, 0, 255));
    assert!(window.draw_if_needed(|renderer| {
        let region = renderer.render(&mut buffer, 200);
        assert_eq!(region.bounding_box_origin(), PhysicalPosition::new(0, 0));
        assert_eq!(region.bounding_box_size(), PhysicalSize::new(170, 190));
    }));
}
