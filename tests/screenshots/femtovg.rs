// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Offscreen shadow regression tests for FemtoVG's shared item renderer.
//! Run with `--features femtovg`; a WGPU adapter is required, but no window or display server.

use std::cell::Cell;
use std::rc::Rc;

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

fn init_femtovg() {
    crate::testing::force_reference_os();
    let instance = wgpu::Instance::default();
    let adapter =
        spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("FemtoVG screenshot tests require a WGPU adapter");
    let (device, queue) =
        spin_on::spin_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("failed to create the FemtoVG test device");
    i_slint_core::platform::set_platform(Box::new(ScreenshotBackend { instance, device, queue }))
        .expect("platform already initialized");
}

fn run_case(name: &str, source: &str) -> Result<(), Box<dyn std::error::Error>> {
    init_femtovg();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiler = slint_interpreter::Compiler::default();
    let compiled =
        spin_on::spin_on(compiler.build_from_source(
            source.into(),
            root.join("cases/basic").join(format!("{name}.slint")),
        ));
    compiled.print_diagnostics();
    assert!(!compiled.has_errors());
    let component = compiled.components().last().unwrap().create()?;
    component.show()?;
    let screenshot = component.window().take_snapshot()?;
    crate::testing::compare_images(
        root.join("references/femtovg/basic").join(format!("{name}.png")).to_str().unwrap(),
        &screenshot,
        Default::default(),
        &crate::testing::TestCaseOptions { base_threshold: 3., ..Default::default() },
    )?;
    Ok(())
}

macro_rules! shadow_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() -> Result<(), Box<dyn std::error::Error>> {
            run_case($name, include_str!(concat!("cases/basic/", $name, ".slint")))
        }
    };
}

shadow_case!(shadow_transparent_fill, "issue-6581-drop-shadow-transparent-fill");
shadow_case!(shadow_painted_alpha, "drop-shadow-painted-alpha");
shadow_case!(shadow_thick_border, "drop-shadow-thick-border");
shadow_case!(shadow_spread, "drop-shadow-spread");
shadow_case!(shadow_per_corner_radius, "drop-shadow-per-corner-radius");

#[test]
fn shadow_tracks_source_paint() {
    init_femtovg();
    crate::shadow::assert_shadow_tracks_source_paint();
}

#[test]
fn shadow_spread_preserves_adjusted_corner_radii() {
    init_femtovg();
    crate::shadow::assert_shadow_spread_preserves_adjusted_corner_radii();
}
