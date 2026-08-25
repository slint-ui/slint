// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for opaque occlusion culling. Requires the `testing` feature (for
//! `rendered_item_count()`) -- run with `--features testing`.
//!
//! Counts how many items `render_item_children`'s tree walk dispatches a draw call for, via
//! `rendered_item_count()`. A scene with many fully hidden siblings behind one opaque covering
//! rectangle should produce a count far below that.
#![cfg(feature = "testing")]

use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_software::{LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType};
use slint_interpreter::{Compiler, ComponentHandle};
use std::rc::Rc;

struct TestPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0 as u64)
    }
}

struct NullLineBuffer;
impl LineBufferProvider for NullLineBuffer {
    type TargetPixel = i_slint_core::graphics::Rgb8Pixel;
    fn process_line(
        &mut self,
        _line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        use i_slint_renderer_software::TargetPixel;
        let mut scratch = vec![Self::TargetPixel::background(); range.len()];
        render_fn(&mut scratch);
    }
}

fn rendered_item_count(source: String) -> usize {
    // NewBuffer always redraws through the plain (non-partial) renderer, bypassing
    // `PartialRenderer` -- and occlusion culling is implemented inside `PartialRenderer`. Use
    // ReusedBuffer so the render actually goes through `PartialRenderer::filter_item`.
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    i_slint_core::platform::set_platform(Box::new(TestPlatform { window: window.clone() }))
        .expect("platform already initialized");

    let compiled =
        spin_on::spin_on(Compiler::default().build_from_source(source, Default::default()));
    assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics().collect::<Vec<_>>());
    let instance = compiled.components().last().expect("no component").create().unwrap();
    instance.show().unwrap();

    window.set_size(i_slint_core::api::PhysicalSize::new(200, 200));
    window.request_redraw();

    i_slint_core::item_rendering::reset_rendered_item_count();
    window.draw_if_needed(|renderer| {
        renderer.render_by_line(NullLineBuffer);
    });
    i_slint_core::item_rendering::rendered_item_count()
}

const HIDDEN_RECT_COUNT: usize = 50;

#[test]
fn opaque_cover_culls_hidden_siblings() {
    let source = format!(
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
    );

    let count = rendered_item_count(source);

    assert!(
        count < HIDDEN_RECT_COUNT,
        "expected the opaque covering rectangle to cull the {HIDDEN_RECT_COUNT} fully hidden \
         siblings behind it, but the tree walk dispatched {count} draw commands",
    );
}

/// The occluder and occluded items live in different subtrees (siblings of each other's
/// *ancestors*, not direct siblings), so a sibling-only occlusion scan could never catch this;
/// region-based occlusion, accumulated over the whole reverse-paint-order walk, should.
///
/// The hidden rectangles are spread out (not stacked) so the only way to cull them is via the
/// opaque rectangle in the sibling branch, not ordinary same-parent self-occlusion.
#[test]
fn opaque_cover_culls_hidden_content_in_a_different_branch() {
    let source = format!(
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
    );

    let count = rendered_item_count(source);

    assert!(
        count < HIDDEN_RECT_COUNT,
        "expected the opaque covering rectangle to cull the {HIDDEN_RECT_COUNT} fully hidden \
         rectangles in the sibling branch, but the tree walk dispatched {count} draw commands",
    );
}

const OVERFLOW_RECT_COUNT: usize = 20;

/// Regression test: a sibling that fully covers a container's own (tiny) bounding rect must not
/// be treated as hiding earlier siblings under that container that are unclipped and overflow far
/// outside its bounds -- absent `clip: true`, a child's paint footprint isn't bounded by its
/// parent's rect.
#[test]
fn unclipped_overflowing_children_are_not_culled() {
    let source = format!(
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
    );

    let count = rendered_item_count(source);

    assert!(
        count >= OVERFLOW_RECT_COUNT,
        "expected the {OVERFLOW_RECT_COUNT} unclipped, overflowing children to remain visible \
         (not incorrectly culled because their tiny container's own bounding box is fully \
         covered by a later sibling), but only {count} draw commands were dispatched",
    );
}

const ROUNDED_CORNER_RECT_COUNT: usize = 25;

/// Regression test: a rounded `clip: true` container masks children to its rounded shape at
/// render time, so an opaque unrounded child filling its rectangular bounds still leaves the
/// rounded-off corners unpainted. Content behind those corners must not be culled.
///
/// Probes sit in [0px, 10px] square, well outside the clip's 90px-radius boundary circle
/// (centered at (90px, 90px)), and are spread out so they don't occlude each other.
#[test]
fn rounded_clip_does_not_cull_content_behind_its_corners() {
    let source = format!(
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
                    // Unrounded, opaque, exactly fills the clip's rectangular bounds -- but the
                    // clip still masks its paint to the rounded shape.
                    x: 0px;
                    y: 0px;
                    width: 200px;
                    height: 200px;
                    background: blue;
                }}
            }}
        }}
        "#
    );

    let count = rendered_item_count(source);

    assert!(
        count >= ROUNDED_CORNER_RECT_COUNT,
        "expected the {ROUNDED_CORNER_RECT_COUNT} probe rectangles behind the rounded clip's \
         cut-off corner to remain visible (not incorrectly culled as if the opaque unrounded \
         child covered the clip's full rectangular bounds), but only {count} draw commands were \
         dispatched",
    );
}
