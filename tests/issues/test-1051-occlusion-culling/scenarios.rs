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
