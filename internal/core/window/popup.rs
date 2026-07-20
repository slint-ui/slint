// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore unclipped

//! Popup window handling helpers

use crate::items::{ConstraintAdjustment, PopupAnchor, PopupAnchorLocation, PopupGravity};
use crate::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use crate::Coord;

/// Returns, as fractions of the anchor rectangle's width/height, the point within that
/// rectangle that the popup is anchored to (0.0 = left/top edge, 0.5 = center, 1.0 =
/// right/bottom edge). Mirrors the Wayland `xdg_positioner` anchor semantics.
fn anchor_location_fraction(location: PopupAnchorLocation) -> (Coord, Coord) {
    match location {
        PopupAnchorLocation::Center => (0.5 as Coord, 0.5 as Coord),
        PopupAnchorLocation::Top => (0.5 as Coord, 0.0 as Coord),
        PopupAnchorLocation::Bottom => (0.5 as Coord, 1.0 as Coord),
        PopupAnchorLocation::Left => (0.0 as Coord, 0.5 as Coord),
        PopupAnchorLocation::Right => (1.0 as Coord, 0.5 as Coord),
        PopupAnchorLocation::TopLeft => (0.0 as Coord, 0.0 as Coord),
        PopupAnchorLocation::BottomLeft => (0.0 as Coord, 1.0 as Coord),
        PopupAnchorLocation::TopRight => (1.0 as Coord, 0.0 as Coord),
        PopupAnchorLocation::BottomRight => (1.0 as Coord, 1.0 as Coord),
    }
}

/// Returns, as fractions of the popup's own width/height, the offset from the anchor point
/// to the popup's origin (top-left corner). For example a gravity of `BottomRight` places the
/// popup's top-left corner at the anchor point, so the popup grows down and to the right.
fn gravity_fraction(gravity: PopupGravity) -> (Coord, Coord) {
    match gravity {
        PopupGravity::Center => (-0.5 as Coord, -0.5 as Coord),
        PopupGravity::Top => (-0.5 as Coord, -1.0 as Coord),
        PopupGravity::Bottom => (-0.5 as Coord, 0.0 as Coord),
        PopupGravity::Left => (-1.0 as Coord, -0.5 as Coord),
        PopupGravity::Right => (0.0 as Coord, -0.5 as Coord),
        PopupGravity::TopLeft => (-1.0 as Coord, -1.0 as Coord),
        PopupGravity::BottomLeft => (-1.0 as Coord, 0.0 as Coord),
        PopupGravity::TopRight => (0.0 as Coord, -1.0 as Coord),
        PopupGravity::BottomRight => (0.0 as Coord, 0.0 as Coord),
    }
}

/// Adjusts a single axis of the popup's placement to stay within `[clip_min, clip_max]`,
/// applying `adj`'s flags in the order the `xdg_positioner` protocol suggests: flip, then
/// slide, then resize. `flipped_origin` is the alternate origin obtained by mirroring both the
/// anchor edge and the gravity on this axis; it is only used when `adj.flip` is set and it
/// actually results in a better fit than the original origin.
fn constrain_axis(
    origin: Coord,
    extent: Coord,
    clip_min: Coord,
    clip_max: Coord,
    flipped_origin: Coord,
    adj: &ConstraintAdjustment,
) -> (Coord, Coord) {
    let fits = |o: Coord| o >= clip_min && o + extent <= clip_max;

    let mut origin = origin;
    if !fits(origin) && adj.flip && fits(flipped_origin) {
        origin = flipped_origin;
    }

    if !fits(origin) && adj.slide {
        // `extent` may exceed the clip size, in which case `clip_max - extent` would be less
        // than `clip_min`; `.max(clip_min)` keeps the clamp range valid in that case.
        let slide_max = (clip_max - extent).max(clip_min);
        origin = origin.clamp(clip_min, slide_max);
    }

    let mut extent = extent;
    if adj.resize && !fits(origin) {
        let clamped_origin = origin.max(clip_min);
        extent = (clip_max - clamped_origin).max(0 as Coord);
        origin = clamped_origin;
    }

    (origin, extent)
}

/// Finds a placement for a popup that has the given `size`, anchored to a rectangle of
/// `anchor.width` x `anchor.height` whose origin is `anchor_position` (in the same coordinate
/// space as `clip_region`), nudged by `offset` (i.e. `anchor.x`/`anchor.y`, the user-facing
/// offset from the anchor point -- not part of the anchor rectangle's geometry), and constrained
/// to stay within `clip_region` according to `anchor.constraint_adjustment_x`/`_y`.
///
/// Mirrors the Wayland `xdg_positioner` placement algorithm: the anchor's `location` selects a
/// point on the anchor rectangle, the `gravity` decides which corner/edge of the popup is placed
/// at that point, and if the resulting rectangle doesn't fit inside `clip_region`, the
/// constraint-adjustment flags decide whether (and how) the popup is moved (`slide`), mirrored to
/// the other side of the anchor point (`flip`), or shrunk (`resize`) to fit. Axes are adjusted
/// independently. If none of the flags are set for an axis, that axis is left as computed even if
/// it doesn't fit, matching the protocol's "none" behavior.
pub fn place_popup(
    anchor: PopupAnchor,
    anchor_position: LogicalPoint,
    offset: LogicalPoint,
    size: LogicalSize,
    clip_region: &LogicalRect,
) -> LogicalRect {
    let (anchor_fx, anchor_fy) = anchor_location_fraction(anchor.location);
    let (gravity_fx, gravity_fy) = gravity_fraction(anchor.gravity);

    let anchor_point_x = anchor_position.x + anchor.width * anchor_fx;
    let anchor_point_y = anchor_position.y + anchor.height * anchor_fy;

    let origin_x = anchor_point_x + size.width * gravity_fx + offset.x;
    let origin_y = anchor_point_y + size.height * gravity_fy + offset.y;

    // Flipping mirrors both the anchor edge and the gravity on that axis, effectively placing
    // the popup on the opposite side of the anchor rectangle.
    let flipped_anchor_point_x = anchor_position.x + anchor.width * (1 as Coord - anchor_fx);
    let flipped_anchor_point_y = anchor_position.y + anchor.height * (1 as Coord - anchor_fy);
    let flipped_x =
        flipped_anchor_point_x + size.width * (-1 as Coord - gravity_fx) + offset.x;
    let flipped_y =
        flipped_anchor_point_y + size.height * (-1 as Coord - gravity_fy) + offset.y;

    let clip_min_x = clip_region.origin.x;
    let clip_max_x = clip_region.origin.x + clip_region.size.width;
    let clip_min_y = clip_region.origin.y;
    let clip_max_y = clip_region.origin.y + clip_region.size.height;

    let (x, width) = constrain_axis(
        origin_x,
        size.width,
        clip_min_x,
        clip_max_x,
        flipped_x,
        &anchor.constraint_adjustment_x,
    );
    let (y, height) = constrain_axis(
        origin_y,
        size.height,
        clip_min_y,
        clip_max_y,
        flipped_y,
        &anchor.constraint_adjustment_y,
    );

    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(width, height))
}

#[cfg(test)]
fn r(x: i32, y: i32, w: i32, h: i32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x as f32, y as f32), LogicalSize::new(w as f32, h as f32))
}

#[cfg(test)]
#[track_caller]
fn fixed_placement(input: LogicalRect, expected: LogicalRect, clip: Option<LogicalRect>) {
    std::eprintln!("fixed: {input:?}, clip({clip:?}) => {expected:?}");
    // A zero-sized anchor rect at `input`'s origin, with TopLeft/BottomRight anchor/gravity,
    // reduces the anchor+gravity math to a no-op, so the popup starts exactly at `input`'s
    // origin -- equivalent to the old "fixed position" placement. When `clip` is `None`, leave
    // all constraint-adjustment flags unset so the position/size pass through unconstrained,
    // regardless of the (unused) clip region passed to `place_popup`.
    let adjustment = ConstraintAdjustment { slide: clip.is_some(), flip: false, resize: clip.is_some() };
    let anchor = PopupAnchor {
        location: PopupAnchorLocation::TopLeft,
        x: input.origin.x,
        y: input.origin.y,
        width: 0.,
        height: 0.,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment_x: adjustment.clone(),
        constraint_adjustment_y: adjustment,
    };
    let clip_region = clip.unwrap_or_else(|| LogicalRect::new(LogicalPoint::zero(), LogicalSize::zero()));
    let result =
        place_popup(anchor, LogicalPoint::new(input.origin.x, input.origin.y), LogicalPoint::zero(), input.size, &clip_region);
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

#[test]
fn test_place_popup_gravity() {
    // A 20x10 anchor rect at (100, 100), popup size 40x30, no constraints applied (unclipped).
    let anchor_position = LogicalPoint::new(100., 100.);
    let anchor_width = 20.;
    let anchor_height = 10.;
    let size = LogicalSize::new(40., 30.);
    let no_adjustment = ConstraintAdjustment { slide: false, flip: false, resize: false };
    let clip = LogicalRect::new(LogicalPoint::zero(), LogicalSize::zero());

    let place = |location: PopupAnchorLocation, gravity: PopupGravity| {
        let anchor = PopupAnchor {
            location,
            x: anchor_position.x,
            y: anchor_position.y,
            width: anchor_width,
            height: anchor_height,
            gravity,
            constraint_adjustment_x: no_adjustment.clone(),
            constraint_adjustment_y: no_adjustment.clone(),
        };
        place_popup(anchor, anchor_position, LogicalPoint::zero(), size, &clip)
    };

    // BottomRight gravity anchored to the anchor's bottom-right corner: the popup's top-left
    // corner sits exactly at the anchor rect's bottom-right corner.
    let result = place(PopupAnchorLocation::BottomRight, PopupGravity::BottomRight);
    assert_eq!(result.origin, LogicalPoint::new(120., 110.));
    assert_eq!(result.size, size);

    // TopLeft gravity anchored to the anchor's top-left corner: the popup's bottom-right
    // corner sits exactly at the anchor rect's top-left corner, so the popup extends up-left.
    let result = place(PopupAnchorLocation::TopLeft, PopupGravity::TopLeft);
    assert_eq!(result.origin, LogicalPoint::new(100. - 40., 100. - 30.));

    // Bottom anchor + Bottom gravity: horizontally centered on the anchor, growing downward
    // from its bottom edge.
    let result = place(PopupAnchorLocation::Bottom, PopupGravity::Bottom);
    assert_eq!(result.origin, LogicalPoint::new(100. + anchor_width / 2. - size.width / 2., 110.));

    // Center anchor + Center gravity centers the popup exactly on the anchor rect's center.
    let result = place(PopupAnchorLocation::Center, PopupGravity::Center);
    let anchor_center = LogicalPoint::new(100. + anchor_width / 2., 100. + anchor_height / 2.);
    assert_eq!(
        result.origin,
        LogicalPoint::new(anchor_center.x - size.width / 2., anchor_center.y - size.height / 2.)
    );
}

#[test]
fn test_place_popup_flip() {
    // Anchor rect hugging the right edge of a 300x300 clip region; with BottomRight gravity the
    // popup would overflow past the right edge, so flipping should place it to the left instead.
    let clip = r(0, 0, 300, 300);
    let anchor_position = LogicalPoint::new(280., 100.);
    let anchor_size = LogicalSize::new(10., 10.);
    let size = LogicalSize::new(50., 50.);

    let flip_only = ConstraintAdjustment { slide: false, flip: true, resize: false };
    let anchor = PopupAnchor {
        location: PopupAnchorLocation::TopRight,
        x: anchor_position.x,
        y: anchor_position.y,
        width: anchor_size.width,
        height: anchor_size.height,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment_x: flip_only.clone(),
        constraint_adjustment_y: flip_only,
    };

    // Without flipping the popup would start at x=290 and end at x=340, past the clip's right
    // edge (300); flipping mirrors both anchor edge and gravity, so it should end up entirely
    // to the left of the anchor rect instead, fully inside the clip region.
    let result = place_popup(anchor, anchor_position, LogicalPoint::zero(), size, &clip);
    assert!(result.origin.x >= 0. && result.origin.x + result.size.width <= 300.);
    assert_eq!(result.size, size);
    // Flipped horizontally: popup's right edge lands on the anchor rect's left edge (x=280).
    assert_eq!(result.origin.x, 280. - size.width);
    // Not flipped vertically: still grows down from the anchor's top edge.
    assert_eq!(result.origin.y, 100.);
}

#[test]
fn test_place_popup_slide() {
    // Anchor near the bottom-right corner of the clip region; sliding (without flipping) should
    // shift the popup back into view while keeping its size and general placement direction.
    let clip = r(0, 0, 300, 300);
    let anchor_position = LogicalPoint::new(280., 280.);
    let size = LogicalSize::new(50., 50.);

    let slide_only = ConstraintAdjustment { slide: true, flip: false, resize: false };
    let anchor = PopupAnchor {
        location: PopupAnchorLocation::BottomRight,
        x: anchor_position.x,
        y: anchor_position.y,
        width: 0.,
        height: 0.,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment_x: slide_only.clone(),
        constraint_adjustment_y: slide_only,
    };

    let result = place_popup(anchor, anchor_position, LogicalPoint::zero(), size, &clip);
    assert_eq!(result.size, size);
    assert_eq!(result.origin, LogicalPoint::new(250., 250.));
}

#[test]
fn test_place_popup_resize() {
    // Popup larger than the clip region on both axes; with only `resize` enabled it should be
    // shrunk (and clamped) to exactly fill the clip region rather than sliding or flipping.
    let clip = r(0, 0, 300, 300);
    let anchor_position = LogicalPoint::new(0., 0.);
    let size = LogicalSize::new(500., 500.);

    let resize_only = ConstraintAdjustment { slide: false, flip: false, resize: true };
    let anchor = PopupAnchor {
        location: PopupAnchorLocation::TopLeft,
        x: anchor_position.x,
        y: anchor_position.y,
        width: 0.,
        height: 0.,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment_x: resize_only.clone(),
        constraint_adjustment_y: resize_only,
    };

    let result = place_popup(anchor, anchor_position, LogicalPoint::zero(), size, &clip);
    assert_eq!(result, clip);
}

#[test]
fn test_place_popup_no_adjustment() {
    // With no constraint-adjustment flags set, an overflowing popup is left exactly where the
    // anchor/gravity math puts it, matching the `xdg_positioner` "none" behavior.
    let clip = r(0, 0, 100, 100);
    let anchor_position = LogicalPoint::new(90., 90.);
    let size = LogicalSize::new(50., 50.);

    let no_adjustment = ConstraintAdjustment { slide: false, flip: false, resize: false };
    let anchor = PopupAnchor {
        location: PopupAnchorLocation::TopLeft,
        x: anchor_position.x,
        y: anchor_position.y,
        width: 0.,
        height: 0.,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment_x: no_adjustment.clone(),
        constraint_adjustment_y: no_adjustment,
    };

    let result = place_popup(anchor, anchor_position, LogicalPoint::zero(), size, &clip);
    assert_eq!(result, LogicalRect::new(anchor_position, size));
}
