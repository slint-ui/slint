// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Offscreen shadow regression tests for FemtoVG's shared item renderer.
//!
//! These tests are `#[ignore]`d by default, so `--all-features` doesn't run them on a CI runner
//! without a WGPU adapter or a driver that tolerates them. Run them deliberately with:
//! `cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots --features femtovg -- --ignored`

use std::cell::Cell;
use std::rc::Rc;
use std::sync::OnceLock;

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_femtovg::FemtoVGWGPURenderer;
use slint_interpreter::ComponentHandle;
use wgpu_30 as wgpu;

struct ScreenshotBackend {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Platform for ScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let renderer = FemtoVGWGPURenderer::new(
            self.instance.clone(),
            self.device.clone(),
            self.queue.clone(),
        )?;
        Ok(Rc::new_cyclic(|self_weak| ScreenshotWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer,
        }))
    }

    // Deterministic screenshots need a frozen clock: returning the current tick keeps every
    // animation at its start value instead of advancing with wall-clock time.
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

struct ScreenshotWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: FemtoVGWGPURenderer,
}

impl WindowAdapter for ScreenshotWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    // The 64x64 fallback only has to get the component through its first layout pass; real
    // geometry lands once `update_window_properties` reports the preferred size, below.
    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(64, 64) } else { self.size.get() }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window.scale_factor()),
        });
        self.size.set(size.to_physical(self.window.scale_factor()));
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            self.size.set(
                properties.layout_constraints().preferred.to_physical(self.window.scale_factor()),
            );
        }
    }
}

/// The WGPU instance, device, and queue shared by every FemtoVG screenshot test.
///
/// `cargo test` runs tests in parallel, each on its own thread; building this stack once and
/// cloning its `Send + Sync` handles into each thread's [`ScreenshotBackend`] avoids requesting
/// several devices from the same adapter concurrently, which crashed Windows CI with
/// `STATUS_ACCESS_VIOLATION`.
struct SharedWgpu {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn shared_wgpu() -> Option<&'static SharedWgpu> {
    static WGPU: OnceLock<Option<SharedWgpu>> = OnceLock::new();
    WGPU.get_or_init(|| {
        let instance = wgpu::Instance::default();
        let adapter =
            spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            spin_on::spin_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some(SharedWgpu { instance, device, queue })
    })
    .as_ref()
}

/// Sets up the FemtoVG test platform on the current thread. Returns `false`, instead of panicking,
/// when no WGPU adapter is available: these tests only run when deliberately requested (see the
/// module doc comment), but still shouldn't fail outright on a runner without a GPU.
fn init_femtovg() -> bool {
    crate::testing::force_reference_os();
    let Some(shared) = shared_wgpu() else {
        eprintln!("skipping: no WGPU adapter available for the FemtoVG screenshot tests");
        return false;
    };
    i_slint_core::platform::set_platform(Box::new(ScreenshotBackend {
        instance: shared.instance.clone(),
        device: shared.device.clone(),
        queue: shared.queue.clone(),
    }))
    .expect("platform already initialized");
    true
}

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub reference_path: std::path::PathBuf,
    pub base_threshold: f32,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    if !init_femtovg() {
        return Ok(());
    }
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let compiler = slint_interpreter::Compiler::default();
    let compiled =
        spin_on::spin_on(compiler.build_from_source(source, testcase.absolute_path.clone()));
    compiled.print_diagnostics();
    assert!(!compiled.has_errors());
    let component = compiled.components().last().unwrap().create()?;
    component.show()?;
    let screenshot = component.window().take_snapshot()?;
    crate::testing::compare_images(
        testcase.reference_path.to_str().unwrap(),
        &screenshot,
        Default::default(),
        &crate::testing::TestCaseOptions {
            base_threshold: testcase.base_threshold,
            ..Default::default()
        },
    )?;
    Ok(())
}

#[test]
#[ignore]
fn shadow_tracks_source_paint() {
    if !init_femtovg() {
        return;
    }
    crate::shadow::assert_shadow_tracks_source_paint();
}

#[test]
#[ignore]
fn shadow_spread_preserves_adjusted_corner_radii() {
    if !init_femtovg() {
        return;
    }
    crate::shadow::assert_shadow_spread_preserves_adjusted_corner_radii();
}
