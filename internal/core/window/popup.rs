// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pupup window handling helpers

use crate::lengths::{LogicalPoint, LogicalRect, LogicalSize};

/// A collection of data that might influence the placement of a `Popup`.
pub enum Placement {
    /// Request a fixed position
    Fixed(LogicalRect),
}

/// Find a placement for the `Popup`, using the provided `Placement`.
/// When a `clip_region` is provided, then the `Popup` will stay within those bounds.
/// The `clip_region` typically is the window or the screen the window is on.
pub fn place_popup(placement: Placement, clip_region: &Option<LogicalRect>) -> LogicalRect {
    match placement {
        Placement::Fixed(rect) => {
            let clip = clip_region.unwrap_or(rect);
            if clip.contains_rect(&rect) {
                rect
            } else {
                let size = LogicalSize::new(
                    crate::Coord::min(rect.size.width, clip.size.width),
                    crate::Coord::min(rect.size.height, clip.size.height),
                );
                let origin = LogicalPoint::new(
                    rect.origin
                        .x
                        .clamp(clip.origin.x, clip.origin.x + clip.size.width - size.width),
                    rect.origin
                        .y
                        .clamp(clip.origin.y, clip.origin.y + clip.size.height - size.height),
                );
                LogicalRect::new(origin, size)
            }
        }
    }
}

#[cfg(test)]
fn r(x: i32, y: i32, w: i32, h: i32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x as f32, y as f32), LogicalSize::new(w as f32, h as f32))
}

#[cfg(test)]
#[track_caller]
fn fixed_placement(input: LogicalRect, expected: LogicalRect, clip: Option<LogicalRect>) {
    std::eprintln!("fixed: {input:?}, clip({clip:?}) => {expected:?}");
    let result = place_popup(Placement::Fixed(input), &clip);
    if let Some(clip) = clip {
        clip.contains_rect(&result);
    }
    assert_eq!(result, expected);
}

#[test]
fn test_place_popup_fixed_unclipped() {
    let data = r(5, 5, 100, 100);
    fixed_placement(data, data, None);

    let data = r(5, -20, 100, 100);
    fixed_placement(data, data, None);
    let data = r(2000, -20, 100, 100);
    fixed_placement(data, data, None);
    let data = r(2000, 5, 100, 100);
    fixed_placement(data, data, None);
    let data = r(2000, 2000, 100, 100);
    fixed_placement(data, data, None);
    let data = r(5, 2000, 100, 100);
    fixed_placement(data, data, None);
    let data = r(-20, 2000, 100, 100);
    fixed_placement(data, data, None);
    let data = r(-20, 5, 100, 100);
    fixed_placement(data, data, None);
    let data = r(-20, -20, 100, 100);
    fixed_placement(data, data, None);

    let data = r(-20, -20, 2000, 2000);
    fixed_placement(data, data, None);
}

#[test]
fn test_place_popup_fixed_clipped() {
    for (clip_offset_x, clip_offset_y) in [
        (-200, -200),
        (-200, 0),
        (-200, 200),
        (0, -200),
        (0, 0),
        (0, 200),
        (200, -200),
        (200, 0),
        (200, 200),
    ] {
        for (clip_width, clip_height) in [(110, 110), (800, 600)] {
            let clip = r(clip_offset_x, clip_offset_y, clip_width, clip_height);

            let x_w = clip_offset_x - 10;
            let x_c = clip_offset_x + 5;
            let x_e = clip_offset_x + clip_width - 80;

            let y_n = clip_offset_y - 10;
            let y_c = clip_offset_y + 5;
            let y_s = clip_offset_y + clip_height - 80;

            let x_min = clip_offset_x;
            let x_max = clip_offset_x + clip_width;
            let y_min = clip_offset_y;
            let y_max = clip_offset_y + clip_height;

            assert!(clip_width > 105 && clip_width < 1000);
            assert!(clip_height > 105 && clip_height < 1000);

            // smaller, inside
            fixed_placement(r(x_c, y_c, 100, 100), r(x_c, y_c, 100, 100), Some(clip));

            // smaller, partial outside
            fixed_placement(r(x_c, y_n, 100, 100), r(x_c, y_min, 100, 100), Some(clip));
            fixed_placement(r(x_e, y_n, 100, 100), r(x_max - 100, y_min, 100, 100), Some(clip));
            fixed_placement(r(x_e, y_c, 100, 100), r(x_max - 100, y_c, 100, 100), Some(clip));
            fixed_placement(
                r(x_e, y_s, 100, 100),
                r(x_max - 100, y_max - 100, 100, 100),
                Some(clip),
            );
            fixed_placement(r(x_c, y_s, 100, 100), r(x_c, y_max - 100, 100, 100), Some(clip));
            fixed_placement(r(x_c, y_s, 100, 100), r(x_c, y_max - 100, 100, 100), Some(clip));
            fixed_placement(r(x_w, y_s, 100, 100), r(x_min, y_max - 100, 100, 100), Some(clip));
            fixed_placement(r(x_w, y_c, 100, 100), r(x_min, y_c, 100, 100), Some(clip));
            fixed_placement(r(x_w, y_n, 100, 100), r(x_min, y_min, 100, 100), Some(clip));

            // smaller, totally outside
            fixed_placement(r(x_c, -2000, 100, 100), r(x_c, y_min, 100, 100), Some(clip));
            fixed_placement(r(2000, -2000, 100, 100), r(x_max - 100, y_min, 100, 100), Some(clip));
            fixed_placement(r(2000, y_c, 100, 100), r(x_max - 100, y_c, 100, 100), Some(clip));
            fixed_placement(
                r(2000, 2000, 100, 100),
                r(x_max - 100, y_max - 100, 100, 100),
                Some(clip),
            );
            fixed_placement(r(x_c, 2000, 100, 100), r(x_c, y_max - 100, 100, 100), Some(clip));
            fixed_placement(r(-2000, 2000, 100, 100), r(x_min, y_max - 100, 100, 100), Some(clip));
            fixed_placement(r(-2000, y_c, 100, 100), r(x_min, y_c, 100, 100), Some(clip));
            fixed_placement(r(-2000, -2000, 100, 100), r(x_min, y_min, 100, 100), Some(clip));

            // matching size, covering
            fixed_placement(
                r(x_min, y_min, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );

            // matching size, overlapping
            fixed_placement(
                r(x_c, y_c, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_c, y_n, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );

            fixed_placement(
                r(x_e, y_n, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_e, y_c, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_e, y_s, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_c, y_s, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_s, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_c, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_n, clip_width, clip_height),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );

            // too big, overlapping
            fixed_placement(
                r(x_c, y_c, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_c, y_n, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_e, y_n, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_e, y_c, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_e, y_s, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_c, y_s, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_s, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_c, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_w, y_n, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );

            // too big, outside
            fixed_placement(
                r(x_c, -3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(3000, -3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(3000, y_c, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(3000, 3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(x_c, 3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(-3000, 3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(-3000, y_c, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
            fixed_placement(
                r(-3000, -3000, clip_width + 5, clip_height + 5),
                r(x_min, y_min, clip_width, clip_height),
                Some(clip),
            );
        }
    }
}
