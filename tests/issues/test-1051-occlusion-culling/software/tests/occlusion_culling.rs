// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for opaque occlusion culling, exercised through the software renderer.
//! `i-slint-core`'s `occlusion-culling` and `testing` features are unconditionally enabled on
//! this package's dev-dependencies (see `Cargo.toml`), so no extra `--features` are needed to
//! run this: `cargo test -p test-occlusion-culling-software`.
//!
//! This is an integration test over the *core* `PartialRenderer` (see
//! `i_slint_core::partial_renderer`), not over the software renderer specifically -- see the
//! Skia-backed twin of this test in `tests/issues/test-1051-occlusion-culling/skia`. The two
//! share their scenes and assertions via `scenarios.rs`, included below by path, and differ only
//! in this module's `rendered_item_count()` harness.
//!
//! Lives in this standalone `tests/` package, rather than in
//! `internal/renderers/software/tests`, so that `i-slint-renderer-software` -- otherwise
//! no_std/MCU-target -- doesn't need `slint-interpreter` (and everything it pulls in: the
//! compiler, the interpreter, the winit backend) as a dev-dependency just to run this.

#[path = "../../scenarios.rs"]
mod scenarios;

use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_software::{LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType};
use slint_interpreter::{Compiler, ComponentHandle, Value};
use std::rc::Rc;

struct TestPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

#[derive(Default)]
struct NullLineBuffer {
    // Reused across `process_line` calls (one per scan line) instead of allocating a fresh `Vec` each time.
    scratch: Vec<i_slint_core::graphics::Rgb8Pixel>,
}
impl LineBufferProvider for NullLineBuffer {
    type TargetPixel = i_slint_core::graphics::Rgb8Pixel;
    fn process_line(
        &mut self,
        _line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        use i_slint_renderer_software::TargetPixel;
        if self.scratch.len() < range.len() {
            self.scratch.resize(range.len(), Self::TargetPixel::background());
        }
        render_fn(&mut self.scratch[..range.len()]);
    }
}

/// Renders the scene once per entry in `frames`, applying that frame's property values before
/// drawing it, and returns how many items each frame dispatched a draw call for.
fn rendered_item_counts(
    source: String,
    scale_factor: f32,
    frames: &[scenarios::Frame],
) -> Vec<usize> {
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

    instance
        .window()
        .dispatch_event(i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    // Logical, not physical: keeps the scene's declared 200x200 logical window size regardless
    // of `scale_factor`, letting the physical framebuffer size scale with it.
    window.set_size(i_slint_core::api::LogicalSize::new(200., 200.));

    frames
        .iter()
        .map(|scenarios::Frame(properties)| {
            for (name, value) in *properties {
                instance.set_property(name, Value::Number(*value as f64)).unwrap();
            }
            // Redraw even when nothing reported itself dirty: a frame that finds an empty dirty
            // region has to draw nothing, and that's exactly what a multi-frame scenario checks.
            window.request_redraw();
            i_slint_core::item_rendering::reset_rendered_item_count();
            window.draw_if_needed(|renderer| {
                renderer.render_by_line(NullLineBuffer::default());
            });
            i_slint_core::item_rendering::rendered_item_count()
        })
        .collect()
}

/// Renders one frame of the scene, without setting any properties first.
fn rendered_item_count(source: String, scale_factor: f32) -> usize {
    rendered_item_counts(source, scale_factor, &[scenarios::Frame(&[])])[0]
}

/// Runs `f` on a fresh OS thread: `set_platform` may only be called once per thread (the
/// context it installs is thread-local), so a test that needs to render more than one scene for
/// comparison must give each render its own thread.
fn render_on_new_thread(f: impl FnOnce() -> usize + Send + 'static) -> usize {
    std::thread::spawn(f).join().expect("rendering thread panicked")
}

#[test]
fn opaque_cover_culls_hidden_siblings() {
    scenarios::assert_hidden_siblings_culled(rendered_item_count(
        scenarios::hidden_siblings_source(),
        1.0,
    ));
}

#[test]
fn opaque_cover_culls_hidden_content_in_a_different_branch() {
    scenarios::assert_hidden_content_in_a_different_branch_culled(rendered_item_count(
        scenarios::hidden_content_in_a_different_branch_source(),
        1.0,
    ));
}

#[test]
fn unclipped_overflowing_children_are_not_culled() {
    scenarios::assert_unclipped_overflowing_children_not_culled(rendered_item_count(
        scenarios::unclipped_overflowing_children_source(),
        1.0,
    ));
}

#[test]
fn rounded_clip_does_not_cull_content_behind_its_corners() {
    let opaque = render_on_new_thread(|| {
        rendered_item_count(scenarios::rounded_clip_source_opaque_occluder(), 1.0)
    });
    let transparent = render_on_new_thread(|| {
        rendered_item_count(scenarios::rounded_clip_source_transparent_occluder(), 1.0)
    });
    scenarios::assert_rounded_clip_does_not_cull_corners(opaque, transparent);
}

#[test]
fn opaque_cover_shrink_is_sound_at_fractional_scale_factor() {
    scenarios::assert_fractional_scale_factor_shrink_is_sound(rendered_item_count(
        scenarios::fractional_scale_factor_source(),
        1.3,
    ));
}

#[test]
fn partially_covered_probe_is_drawn() {
    scenarios::assert_partially_covered_probe_is_drawn(rendered_item_count(
        scenarios::partially_covered_probe_source(),
        1.0,
    ));
}

#[test]
fn non_covering_occluders_cull_nothing() {
    for (occluder_style, declarations) in scenarios::NON_COVERING_OCCLUDERS {
        let count = render_on_new_thread(|| {
            rendered_item_count(scenarios::occluder_style_source(declarations), 1.0)
        });
        scenarios::assert_occluder_style_culls_nothing(occluder_style, count);
    }
}

#[test]
fn covering_occluder_culls_everything_behind_it() {
    scenarios::assert_covering_occluder_style_culls_everything(rendered_item_count(
        scenarios::occluder_style_source(scenarios::COVERING_OCCLUDER),
        1.0,
    ));
}

#[test]
fn occluder_raised_by_z_culls_the_probe_below_it() {
    scenarios::assert_z_order_raised_occluder_culls(rendered_item_count(
        scenarios::z_order_raised_occluder_source(),
        1.0,
    ));
}

#[test]
fn probe_raised_by_z_above_the_occluder_is_not_culled() {
    scenarios::assert_z_order_raised_probe_is_drawn(rendered_item_count(
        scenarios::z_order_raised_probe_source(),
        1.0,
    ));
}

/// Software-only: the Skia harness renders through `take_snapshot`, which redraws the whole
/// window every time, so a frame that has nothing to redraw can't be observed there.
#[test]
fn moving_occluder_culls_only_while_it_covers() {
    scenarios::assert_moving_occluder_counts(&rendered_item_counts(
        scenarios::moving_occluder_source(),
        1.0,
        scenarios::MOVING_OCCLUDER_FRAMES,
    ));
}
