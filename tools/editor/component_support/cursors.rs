// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::ui::EditorCursors;
use std::collections::HashMap;

pub fn setup(cursors_global: &EditorCursors<'_>) {
    let cursors = std::cell::RefCell::new(HashMap::<i32, slint::Image>::new());
    cursors_global.on_rotation_image(move |angle| {
        let angle = angle.round().rem_euclid(360.0) as i32;
        cursors
            .borrow_mut()
            .entry(angle)
            .or_insert_with(|| {
                let svg = include_str!("../ui/assets/cursors/rotate.svg")
                    .replace("{angle}", &angle.to_string());
                let image = slint::Image::load_from_svg_data(svg.as_bytes())
                    .expect("valid rotation cursor SVG");
                // Match the fixed-pixel canvas pointer; native SVG cursors scale with the display.
                slint::Image::from_rgba8(image.to_rgba8().expect("rotation cursor pixels"))
            })
            .clone()
    });
}
