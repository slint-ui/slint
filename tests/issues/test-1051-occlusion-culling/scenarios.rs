// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `.slint` scenes and assertions shared between the software- and Skia-backed occlusion-culling
//! regression tests (`tests/issues/test-1051-occlusion-culling/software` and
//! `tests/issues/test-1051-occlusion-culling/skia`),
//! included via `#[path]` rather than duplicated: the two packages differ only in which renderer
//! backend's `rendered_item_count()` harness they feed these scenes into.
//!
//! Scenes are fully deterministic, so expected counts are asserted exactly rather than with an
//! inequality. Counts are the number of items `render_item_children`'s tree walk dispatches a
//! draw call for, via `i_slint_core::item_rendering::rendered_item_count()`.

// A few scenes only apply to one of the two harnesses, so the other one includes them unused.
#![allow(dead_code)]

pub const HIDDEN_RECT_COUNT: usize = 50;

pub fn hidden_siblings_source() -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            for i in {HIDDEN_RECT_COUNT}: Rectangle {{
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                background: red;
            }}
            Rectangle {{
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                background: blue;
            }}
        }}
        "#
    )
}

pub fn assert_hidden_siblings_culled(count: usize) {
    // Only the opaque covering rectangle should still be drawn: all 50 hidden siblings behind
    // it are culled.
    assert_eq!(
        count, 1,
        "expected the opaque covering rectangle to cull all {HIDDEN_RECT_COUNT} fully hidden \
         siblings behind it, leaving only the covering rectangle itself drawn, but the tree \
         walk dispatched {count} draw commands",
    );
}

/// The occluder and occluded items live in different subtrees (siblings of each other's
/// *ancestors*, not direct siblings), so a sibling-only occlusion scan could never catch this;
/// region-based occlusion, accumulated over the whole reverse-paint-order walk, should.
///
/// The hidden rectangles are spread out (not stacked) so the only way to cull them is via the
/// opaque rectangle in the sibling branch, not ordinary same-parent self-occlusion.
pub fn hidden_content_in_a_different_branch_source() -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            Rectangle {{
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                for i in {HIDDEN_RECT_COUNT}: Rectangle {{
                    x: mod(i, 10) * 20px;
                    y: (i / 10) * 20px;
                    width: 20px;
                    height: 20px;
                    background: red;
                }}
            }}
            Rectangle {{
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                Rectangle {{
                    x: 0px;
                    y: 0px;
                    width: 200px;
                    height: 200px;
                    background: blue;
                }}
            }}
        }}
        "#
    )
}

pub fn assert_hidden_content_in_a_different_branch_culled(count: usize) {
    // As above: only the opaque covering leaf survives; both wrapping containers and all 50
    // probes are culled.
    assert_eq!(
        count, 1,
        "expected the opaque covering rectangle to cull all {HIDDEN_RECT_COUNT} fully hidden \
         rectangles in the sibling branch, leaving only the covering rectangle itself drawn, but \
         the tree walk dispatched {count} draw commands",
    );
}

pub const OVERFLOW_RECT_COUNT: usize = 20;

/// Regression test: a sibling that fully covers a container's own (tiny) bounding rect must not
/// be treated as hiding earlier siblings under that container that are unclipped and overflow far
/// outside its bounds -- absent `clip: true`, a child's paint footprint isn't bounded by its
/// parent's rect.
pub fn unclipped_overflowing_children_source() -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            Rectangle {{
                // Tiny non-clipping container: its own bounding rect is (0,0,10,10).
                x: 0px;
                y: 0px;
                width: 10px;
                height: 10px;
                for i in {OVERFLOW_RECT_COUNT}: Rectangle {{
                    // Declared (thus painted) before the opaque cover below, and overflowing
                    // far outside the container's own bounds. Spread out along x so they don't
                    // stack on top of (and thus legitimately occlude) each other.
                    x: 100px + i * 5px;
                    y: 100px;
                    width: 4px;
                    height: 4px;
                    background: green;
                }}
                Rectangle {{
                    // Declared (thus painted) last: exactly covers the container's own bounding
                    // rect, but not the overflowing siblings above.
                    x: 0px;
                    y: 0px;
                    width: 10px;
                    height: 10px;
                    background: blue;
                }}
            }}
        }}
        "#
    )
}

pub fn assert_unclipped_overflowing_children_not_culled(count: usize) {
    // The `OVERFLOW_RECT_COUNT` overflowing probes, plus the container and the covering
    // rectangle -- neither of which is occluded, since the container's own footprint is larger
    // than what its covering child alone can claim as occluded once the overflowing probes are
    // also accounted for.
    let expected = OVERFLOW_RECT_COUNT + 2;
    assert_eq!(
        count, expected,
        "expected the {OVERFLOW_RECT_COUNT} unclipped, overflowing children to remain visible \
         (not incorrectly culled because their tiny container's own bounding box is fully \
         covered by a later sibling), plus the container and the covering rectangle itself, for \
         {expected} draw commands total, but the tree walk dispatched {count}",
    );
}

pub const ROUNDED_CORNER_RECT_COUNT: usize = 25;

/// Regression test: a rounded `clip: true` container masks children to its rounded shape at
/// render time, so an opaque unrounded child filling its rectangular bounds still leaves the
/// rounded-off corners unpainted. Content behind those corners must not be culled.
///
/// Probes sit in [0px, 10px] square, well outside the clip's 90px-radius boundary circle
/// (centered at (90px, 90px)), and are spread out so they don't occlude each other.
///
/// `occluder_background` is `blue` for the real regression scenario and `transparent` for the
/// control scenario: comparing the two draw-item counts (see
/// `assert_rounded_clip_does_not_cull_corners`) proves the opaque occluder culls nothing extra,
/// without hardcoding how many items `clip: true` + `border-radius` happens to lower to today --
/// that's a compiler detail, and one master is actively changing.
fn rounded_clip_source(occluder_background: &str) -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            for i in {ROUNDED_CORNER_RECT_COUNT}: Rectangle {{
                // 5x5 grid of 2px cells tiling [0,10)x[0,10), deep in the excluded corner.
                x: mod(i, 5) * 2px;
                y: (i / 5) * 2px;
                width: 2px;
                height: 2px;
                background: green;
            }}
            Rectangle {{
                // Declared (thus painted) last, on top of the probes above.
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                clip: true;
                border-radius: 90px;

                Rectangle {{
                    // Unrounded, exactly fills the clip's rectangular bounds -- but the clip
                    // still masks its paint to the rounded shape.
                    x: 0px;
                    y: 0px;
                    width: 200px;
                    height: 200px;
                    background: {occluder_background};
                }}
            }}
        }}
        "#
    )
}

pub fn rounded_clip_source_opaque_occluder() -> String {
    rounded_clip_source("blue")
}

pub fn rounded_clip_source_transparent_occluder() -> String {
    rounded_clip_source("transparent")
}

pub fn assert_rounded_clip_does_not_cull_corners(opaque_count: usize, transparent_count: usize) {
    // Same scene either way, so the item count the tree walk dispatches draw calls for should be
    // identical whether the corner-filling rectangle is opaque or transparent: the
    // `ROUNDED_CORNER_RECT_COUNT` probes behind the cut-off corners must not become collateral
    // damage of the opaque occluder, and the transparent variant establishes the baseline count
    // without needing to know how many items the rounded `clip: true` container itself lowers to.
    assert_eq!(
        opaque_count, transparent_count,
        "expected the {ROUNDED_CORNER_RECT_COUNT} probe rectangles behind the rounded clip's \
         cut-off corner to remain visible (not incorrectly culled as if the opaque unrounded \
         child covered the clip's full rectangular bounds): the opaque-occluder variant \
         dispatched {opaque_count} draw commands, but the transparent-occluder control \
         dispatched {transparent_count}",
    );
}

/// Regression test for the anti-aliasing fix: an occluder's logical edge that lands on a
/// fractional device pixel at a non-integral `scale_factor` only partially covers that boundary
/// pixel, so the occluder's claimed region must shrink to the device-pixel interior rather than
/// its full logical bounds. Exercised at `scale_factor` 1.3.
///
/// The occluder's right edge sits at logical x=101.53, which at scale 1.3 lands at physical
/// x=131.989 -- inside the pixel column [131, 132), which is therefore only partially covered.
/// Shrinking to the device-pixel interior claims only up to physical x=131 (logical
/// x=131/1.3=100.769...), leaving a ~0.76-logical-pixel-wide sliver, [100.769, 101.53), that must
/// not be claimed as occluded. `sliver_probe` sits entirely inside that sliver; `deep_probe` sits
/// well inside the safely-claimed region and must still be culled.
///
/// Unlike the other scenarios, the occluder here only covers part of the window (it needs a
/// free edge to land on a fractional device pixel), so the window's own implicit background --
/// which none of these three rectangles fully covers -- stays unoccluded and drawn, adding a
/// constant +1 to the count. See `assert_fractional_scale_factor_shrink_is_sound`.
pub fn fractional_scale_factor_source() -> String {
    r#"
    export component AppWindow inherits Window {
        width: 200px;
        height: 200px;
        Rectangle {
            // Deep probe: well inside the claimed occluded region, must still be culled.
            x: 10px;
            y: 10px;
            width: 4px;
            height: 4px;
            background: red;
        }
        Rectangle {
            // Sliver probe: inside the sub-pixel sliver the shrink must leave unclaimed.
            x: 100.9px;
            y: 10px;
            width: 0.5px;
            height: 4px;
            background: green;
        }
        Rectangle {
            // Declared (thus painted) last, on top of the probes above. Its right edge (see
            // above) lands on a fractional device pixel at scale_factor 1.3.
            x: 0px;
            y: 0px;
            width: 101.53px;
            height: 200px;
            background: blue;
        }
    }
    "#
    .into()
}

pub fn assert_fractional_scale_factor_shrink_is_sound(count: usize) {
    // The occluder, the window's own implicit (unoccluded) background, and the sliver probe are
    // drawn; the deep probe is culled.
    //
    // Only pins the shrink's direction, not its magnitude: a shrink that gives up more margin
    // than necessary (but still stays clear of the sliver probe) leaves the same draw count,
    // so this only catches the shrink going the wrong way, not a wasteful one.
    assert_eq!(
        count, 3,
        "expected the occluder, the window's own background, and the sliver probe sitting in \
         the sub-pixel-wide gap the device-pixel shrink must leave unclaimed to all be drawn (3 \
         draw commands), with only the deep probe culled, but the tree walk dispatched {count}",
    );
}

/// A window-filling occluder painted over a single probe rectangle, with `occluder_declarations`
/// spliced into the occluder. Used to check which occluder styles may claim their rectangle as
/// opaquely covered (see `NON_COVERING_OCCLUDERS`).
pub fn occluder_style_source(occluder_declarations: &str) -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            Rectangle {{
                x: 10px;
                y: 10px;
                width: 4px;
                height: 4px;
                background: red;
            }}
            Rectangle {{
                // Declared (thus painted) last, on top of the probe above.
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                {occluder_declarations}
            }}
        }}
        "#
    )
}

/// Occluder styles that don't opaquely cover every pixel of their own rectangle, so nothing
/// behind them may be culled. Each entry is a name for the assertion message and the
/// declarations to splice into `occluder_style_source`.
pub const NON_COVERING_OCCLUDERS: &[(&str, &str)] = &[
    ("a semi-transparent background", "background: #0000ff80;"),
    (
        "a gradient with a transparent stop",
        "background: @linear-gradient(0deg, #0000ff80 0%, blue 100%);",
    ),
    ("rounded corners", "background: blue; border-radius: 20px;"),
];

/// The opaque, unrounded occluder that `NON_COVERING_OCCLUDERS` is compared against.
pub const COVERING_OCCLUDER: &str = "background: blue;";

pub fn assert_occluder_style_culls_nothing(occluder_style: &str, count: usize) {
    // The window's own background, the probe, and the occluder itself.
    assert_eq!(
        count, 3,
        "expected an occluder with {occluder_style} to cull nothing, leaving the window's own \
         background, the probe rectangle behind it and the occluder itself to be drawn (3 draw \
         commands), but the tree walk dispatched {count}",
    );
}

pub fn assert_covering_occluder_style_culls_everything(count: usize) {
    // Establishes that `occluder_style_source`'s scene does cull once the occluder is opaque and
    // unrounded, so `assert_occluder_style_culls_nothing` isn't passing vacuously.
    assert_eq!(
        count, 1,
        "expected the opaque, unrounded occluder to cull both the probe rectangle behind it and \
         the window's own background, leaving only the occluder itself drawn, but the tree walk \
         dispatched {count} draw commands",
    );
}

/// An occluder covering only part of the probe behind it, which is the ordinary case: neither
/// item may be culled.
pub fn partially_covered_probe_source() -> String {
    r#"
    export component AppWindow inherits Window {
        width: 200px;
        height: 200px;
        Rectangle {
            x: 0px;
            y: 0px;
            width: 40px;
            height: 40px;
            background: red;
        }
        Rectangle {
            // Declared (thus painted) last, covering the probe's right half and everything to
            // the right of it, but leaving [0px, 20px) of the window uncovered.
            x: 20px;
            y: 0px;
            width: 180px;
            height: 200px;
            background: blue;
        }
    }
    "#
    .into()
}

pub fn assert_partially_covered_probe_is_drawn(count: usize) {
    // The window's own background, the partly covered probe, and the occluder.
    assert_eq!(
        count, 3,
        "expected a probe that the occluder only partly covers to still be drawn, along with the \
         window's own background (also only partly covered) and the occluder itself, for 3 draw \
         commands, but the tree walk dispatched {count}",
    );
}

/// Paint order follows the `z` property, not declaration order, so occlusion has to as well.
/// `raised` is the element whose `z` is set to 1, lifting it above its sibling regardless of
/// which of the two is declared first.
fn z_order_source(raised: &str) -> String {
    let (probe_z, occluder_z) = match raised {
        "probe" => ("z: 1;", ""),
        "occluder" => ("", "z: 1;"),
        other => unreachable!("unknown raised element: {other}"),
    };
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            Rectangle {{
                x: 10px;
                y: 10px;
                width: 4px;
                height: 4px;
                background: red;
                {probe_z}
            }}
            Rectangle {{
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                background: blue;
                {occluder_z}
            }}
        }}
        "#
    )
}

/// The occluder is declared first, so only its `z: 1` puts it in front of the probe.
pub fn z_order_raised_occluder_source() -> String {
    z_order_source("occluder")
}

/// The occluder is declared last, but the probe's `z: 1` keeps it in front of the occluder.
pub fn z_order_raised_probe_source() -> String {
    z_order_source("probe")
}

pub fn assert_z_order_raised_occluder_culls(count: usize) {
    assert_eq!(
        count, 1,
        "expected the occluder raised above the probe with 'z: 1' to cull both the probe and the \
         window's own background, leaving only the occluder itself drawn, but the tree walk \
         dispatched {count} draw commands",
    );
}

pub fn assert_z_order_raised_probe_is_drawn(count: usize) {
    // The probe and the occluder; the window's own background is culled by the occluder, which
    // covers it opaquely whichever of the two siblings ends up in front.
    assert_eq!(
        count, 2,
        "expected the probe raised above the occluder with 'z: 1' to be drawn along with the \
         occluder itself, for 2 draw commands, but the tree walk dispatched {count}",
    );
}

/// A rotated occluder covers a diamond, not its bounding rectangle, so it may claim nothing:
/// the probe sits in a corner the rotation leaves uncovered.
///
/// Only meaningful on a renderer that supports transformations. The software renderer doesn't
/// (`ItemRendererFeatures::SUPPORTS_TRANSFORMATIONS` is false there), so it paints the occluder
/// unrotated -- and then culling the probe behind it is correct.
///
/// `occluder_background` is `blue` for the real scenario and `transparent` for the control, whose
/// draw-item count establishes the baseline without hardcoding how many items the rotation
/// lowers to (see `assert_rotated_occluder_culls_nothing`).
fn rotated_occluder_source(occluder_background: &str) -> String {
    format!(
        r#"
        export component AppWindow inherits Window {{
            width: 200px;
            height: 200px;
            Rectangle {{
                // Top-left corner, outside the rotated occluder's diamond.
                x: 2px;
                y: 2px;
                width: 4px;
                height: 4px;
                background: red;
            }}
            Rectangle {{
                // Declared (thus painted) last, on top of the probe above. Rotated by 45
                // degrees about its center, so its corners no longer cover the window's.
                x: 0px;
                y: 0px;
                width: 200px;
                height: 200px;
                background: {occluder_background};
                transform-rotation: 45deg;
            }}
        }}
        "#
    )
}

pub fn rotated_occluder_source_opaque() -> String {
    rotated_occluder_source("blue")
}

pub fn rotated_occluder_source_transparent() -> String {
    rotated_occluder_source("transparent")
}

pub fn assert_rotated_occluder_culls_nothing(opaque_count: usize, transparent_count: usize) {
    // Same scene either way, so the draw-item count should be identical whether the rotated
    // occluder is opaque or transparent: neither the probe in the uncovered corner nor the
    // window's own background may be culled.
    assert_eq!(
        opaque_count, transparent_count,
        "expected the rotated occluder to cull nothing (it covers a diamond, not the rectangle \
         the probe's corner sits in): the opaque variant dispatched {opaque_count} draw \
         commands, but the transparent control dispatched {transparent_count}",
    );
}

/// The `in` property values to set on a scene before drawing one frame of a multi-frame
/// scenario, as name/value pairs; the value is a plain number (a length in logical pixels for a
/// `length` property).
pub struct Frame(pub &'static [(&'static str, f32)]);

/// Occlusion is recomputed every frame, so an occluder that moves off a probe must let it be
/// drawn again, and one that moves onto a probe must cull it from then on.
///
/// Also a regression test for an occluded item keeping a dirty rendering tracker: `tint` changes
/// the probe's color in the same frame the occluder covers it, so the probe goes into hiding
/// dirty. Its tracker has to be dropped, or it stays dirty forever and marks its rectangle dirty
/// on every later frame -- which is what frame 3, drawing nothing at all, pins down.
pub fn moving_occluder_source() -> String {
    r#"
    export component AppWindow inherits Window {
        width: 200px;
        height: 200px;
        in property <length> cover-x;
        in property <int> tint;
        Rectangle {
            x: 10px;
            y: 10px;
            width: 4px;
            height: 4px;
            background: rgb(root.tint, 0, 0);
        }
        Rectangle {
            // Declared (thus painted) last, on top of the probe above.
            x: root.cover-x;
            y: 0px;
            width: 200px;
            height: 200px;
            background: blue;
        }
    }
    "#
    .into()
}

/// The frames to draw for `moving_occluder_source`, in order. `cover-x` 210px puts the occluder
/// entirely outside the 200px-wide window.
pub const MOVING_OCCLUDER_FRAMES: &[Frame] = &[
    Frame(&[("cover-x", 210.), ("tint", 0.)]),
    Frame(&[("cover-x", 0.), ("tint", 200.)]),
    Frame(&[]),
    Frame(&[("cover-x", 210.)]),
];

pub fn assert_moving_occluder_counts(counts: &[usize]) {
    // Frame 1: the occluder is off-window, so the window's own background and the probe are
    //          drawn, and the occluder itself is clipped away.
    // Frame 2: the occluder has moved over everything and culls both.
    // Frame 3: nothing changed and nothing is dirty, so nothing is drawn -- provided frame 2
    //          dropped the rendering tracker of the probe it hid while it was dirty.
    // Frame 4: the occluder is off-window again, so the probe is drawn once more, with the
    //          color it was given back in frame 2, along with the window's own background.
    assert_eq!(
        counts,
        [2, 1, 0, 2],
        "expected the moving occluder to cull the probe only while it covers it, and the frame \
         after it starts covering it to draw nothing at all, but the tree walk dispatched \
         {counts:?} draw commands per frame",
    );
}
