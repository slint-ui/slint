// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for opaque occlusion culling, exercised through Skia's headless CPU/raster
//! backend (`SkiaRenderer::default_software`, no GPU required). `i-slint-core`'s
//! `occlusion-culling` and `testing` features are unconditionally enabled on this package's
//! dev-dependencies (see `Cargo.toml`), so no extra `--features` are needed to run this:
//! `cargo test -p test-occlusion-culling-skia`.
//!
//! This is an integration test over the *core* `PartialRenderer` (see
//! `i_slint_core::partial_renderer`), not over Skia specifically -- see the software-backed twin
//! of this test in `tests/issues/test-1051-occlusion-culling/software`. The two share their
//! scenes and assertions via `scenarios.rs`, included below by path, and differ only in this
//! module's `rendered_item_count()` harness.
//!
//! Lives in this standalone `tests/` package, rather than in `internal/renderers/skia/tests`, so
//! that `i-slint-renderer-skia` doesn't need `slint-interpreter` (and everything it pulls in: the
//! compiler, the interpreter, the winit backend) as a dev-dependency just to run this.

#[path = "../../scenarios.rs"]
mod scenarios;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use slint_interpreter::{Compiler, ComponentHandle};
use std::cell::Cell;
use std::rc::Rc;

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| TestWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer: SkiaRenderer::default_software(&SkiaSharedContext::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

struct TestWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: SkiaRenderer,
}

impl WindowAdapter for TestWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()))
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}

fn rendered_item_count(source: String, scale_factor: f32) -> usize {
    i_slint_core::platform::set_platform(Box::new(TestPlatform))
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
    instance.window().set_size(i_slint_core::api::LogicalSize::new(200., 200.));

    i_slint_core::item_rendering::reset_rendered_item_count();
    instance.window().take_snapshot().unwrap();
    i_slint_core::item_rendering::rendered_item_count()
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

/// Skia-only: the software renderer reports no transformation support, so it paints the
/// occluder unrotated and culling behind it is correct there (see
/// `scenarios::rotated_occluder_source`).
#[test]
fn rotated_occluder_culls_nothing() {
    let opaque = render_on_new_thread(|| {
        rendered_item_count(scenarios::rotated_occluder_source_opaque(), 1.0)
    });
    let transparent = render_on_new_thread(|| {
        rendered_item_count(scenarios::rotated_occluder_source_transparent(), 1.0)
    });
    scenarios::assert_rotated_occluder_culls_nothing(opaque, transparent);
}
