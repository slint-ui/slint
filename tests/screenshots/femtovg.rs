// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Offscreen screenshot tests for FemtoVG's shared item renderer.
//!
//! They run with the other screenshot tests when the `femtovg` feature is enabled, and pass without
//! rendering on a machine without a WGPU adapter.

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
    wgpu: &'static SharedWgpu,
}

impl Platform for ScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let renderer = FemtoVGWGPURenderer::new(
            self.wgpu.instance.clone(),
            self.wgpu.device.clone(),
            self.wgpu.queue.clone(),
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
        i_slint_core::animations::current_tick().into()
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
/// `cargo test` runs tests in parallel, each on its own thread. Initialize the device once and
/// share it with each thread's [`ScreenshotBackend`].
struct SharedWgpu {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    is_warp: bool,
}

fn shared_wgpu() -> Option<&'static SharedWgpu> {
    static WGPU: OnceLock<Option<SharedWgpu>> = OnceLock::new();
    WGPU.get_or_init(|| {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        // Match Slint's backend selection. Initializing both DX12 and Mesa's OpenGL driver
        // crashes inside D3D12 on Windows CI before an adapter can be requested.
        descriptor.backends = i_slint_core::graphics::wgpu_30::mask_backends(
            descriptor.backends,
            i_slint_core::graphics::wgpu_30::default_backends_to_avoid(),
        );
        let instance = wgpu::Instance::new(descriptor);
        // wgpu's native backends resolve these requests on the first poll.
        let adapter = crate::testing::poll_once(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
        )
        .expect("adapter request is ready")
        .ok();
        // Written to stderr directly, which libtest doesn't capture, so a CI log shows whether
        // these tests rendered anything.
        let report = match &adapter {
            Some(adapter) => {
                let info = adapter.get_info();
                format!("FemtoVG screenshot tests: {} ({:?})\n", info.name, info.backend)
            }
            None => "FemtoVG screenshot tests: skipped, no WGPU adapter\n".into(),
        };
        let _ = std::io::Write::write_all(&mut std::io::stderr(), report.as_bytes());
        let adapter = adapter?;
        let info = adapter.get_info();
        // WARP, the D3D12 software adapter, reports Microsoft's PCI vendor ID.
        let is_warp = info.vendor == 0x1414 && info.device_type == wgpu::DeviceType::Cpu;
        let (device, queue) =
            crate::testing::poll_once(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device request is ready")
                .ok()?;
        Some(SharedWgpu { instance, device, queue, is_warp })
    })
    .as_ref()
}

/// Sets up the FemtoVG test platform on the current thread, or returns `None` when no WGPU
/// adapter is available.
fn init_femtovg() -> Option<&'static SharedWgpu> {
    crate::testing::force_reference_os();
    let wgpu = shared_wgpu()?;
    i_slint_core::platform::set_platform(Box::new(ScreenshotBackend { wgpu }))
        .expect("platform already initialized");
    i_slint_backend_testing::configure_test_fonts();
    Some(wgpu)
}

/// The lowest `base_threshold` for a FemtoVG case.
///
/// GPU rasterizers round anti-aliasing and gradient interpolation differently: WARP, the D3D12
/// software adapter on Windows CI, puts channels up to 2 levels off references made with Metal.
const MIN_BASE_THRESHOLD: f32 = 4.;

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub reference_path: std::path::PathBuf,
    pub base_threshold: f32,
    pub size: PhysicalSize,
    pub scale_factor: f32,
    pub skip_warp: bool,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    let Some(wgpu) = init_femtovg() else {
        return Ok(());
    };
    if testcase.skip_warp && wgpu.is_warp {
        return Ok(());
    }
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let compiled = crate::testing::poll_once(
        compiler.build_from_source(source, testcase.absolute_path.clone()),
    )
    .unwrap();
    compiled.print_diagnostics();
    assert!(!compiled.has_errors());
    let component = compiled.components().last().unwrap().create()?;
    component.window().dispatch_event(i_slint_core::platform::WindowEvent::ScaleFactorChanged {
        scale_factor: testcase.scale_factor,
    });
    component.window().set_size(testcase.size);
    component.show()?;
    let screenshot = component.window().take_snapshot()?;
    crate::testing::compare_images(
        testcase.reference_path.to_str().unwrap(),
        &screenshot,
        Default::default(),
        &crate::testing::TestCaseOptions {
            base_threshold: testcase.base_threshold.max(MIN_BASE_THRESHOLD),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[test]
fn shadow_tracks_source_paint() {
    if init_femtovg().is_none() {
        return;
    }
    crate::shadow::assert_shadow_tracks_source_paint();
}

#[test]
fn shadow_spread_preserves_adjusted_corner_radii() {
    if init_femtovg().is_none() {
        return;
    }
    crate::shadow::assert_shadow_spread_preserves_adjusted_corner_radii();
}
