// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! Helpers available to the `` ```rust `` test code of the Slint SC test cases.
//! The driver includes this module in every generated test program.

#![allow(dead_code, unused_macros)]

/// Render the component and write the screenshot to a file in the current
/// directory, for the driver to compare against the PNG reference afterwards.
/// The optional second argument distinguishes multiple screenshots of the same
/// test (e.g. `screenshot!(x, after_click)`).
macro_rules! screenshot {
    ($component:expr) => {
        crate::harness::save_screenshot(|w, h, buf| $component.render_rgb8(w, h, buf), None)
    };
    ($component:expr, $state:ident) => {
        crate::harness::save_screenshot(
            |w, h, buf| $component.render_rgb8(w, h, buf),
            Some(stringify!($state)),
        )
    };
}

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

pub fn save_screenshot(render: impl FnOnce(u32, u32, &mut [u8]), state: Option<&str>) {
    let mut buffer = [0u8; (WIDTH * HEIGHT * 3) as usize];
    render(WIDTH, HEIGHT, &mut buffer);
    let name = std::env::var("SLINT_TEST_NAME").expect("SLINT_TEST_NAME not set");
    let filename = match state {
        Some(state) => format!("{name}-{state}.ppm"),
        None => format!("{name}.ppm"),
    };
    let mut data = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    data.extend_from_slice(&buffer);
    std::fs::write(&filename, data)
        .unwrap_or_else(|e| panic!("failed to write screenshot {filename}: {e}"));
}
