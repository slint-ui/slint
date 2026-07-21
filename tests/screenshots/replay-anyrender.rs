// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Replay a recorded anyrender command stream through the real
//! `anyrender_vello_cpu` renderer and write the result as a PNG.
//!
//! This is a developer aid for verifying that the captured command
//! stream is faithful: render it through a production backend and
//! compare against the existing `references/skia/<case>.png` or
//! `references/software/<case>.png`.
//!
//! The case identifier is the path of the `.slint` test under
//! `tests/screenshots/cases/`, without the `.slint` suffix.
//!
//! Usage:
//!     replay-anyrender <case>          # writes /tmp/replay-<case>.png
//!     replay-anyrender <case> <out>    # writes <out>
//!
//! Example:
//!     cargo run -p test-driver-screenshots --features anyrender \
//!         --bin replay-anyrender -- text/text
//!     compare /tmp/replay-text-text.png \
//!         tests/screenshots/references/skia/text/text.png \
//!         /tmp/diff.png

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use anyrender::ImageRenderer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_anyrender::{AnyrenderSlintRenderer, RecordingWindowRenderer};
use slint_interpreter::ComponentHandle;

type RecordingRenderer = AnyrenderSlintRenderer<RecordingWindowRenderer>;

/// The recording renderer is created in main() and injected here, keeping a
/// handle to the concrete type after core takes ownership of the window
/// adapter behind `Rc<dyn WindowAdapter>`.
struct ReplayBackend {
    renderer: Rc<RecordingRenderer>,
}

impl Platform for ReplayBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| ReplayWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer: self.renderer.clone(),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

struct ReplayWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: Rc<RecordingRenderer>,
}

impl WindowAdapter for ReplayWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(64, 64) } else { self.size.get() }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()))
    }

    fn renderer(&self) -> &dyn Renderer {
        &*self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }
}

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    let future = std::pin::pin!(future);
    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let case = args.next().ok_or(
        "missing case argument. usage: replay-anyrender <case> [output.png]\n\
         example: replay-anyrender text/text",
    )?;
    let case_clean = case.trim_end_matches(".slint");
    let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let case_path = manifest_dir.join("cases").join(format!("{case_clean}.slint"));
    if !case_path.exists() {
        return Err(format!("case .slint not found at {}", case_path.display()).into());
    }

    let output_path: PathBuf = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(format!("/tmp/replay-{}.png", case_clean.replace('/', "-")))
    });

    // Same as testing::force_reference_os() in the test driver: keep the
    // rendered output independent of the host OS.
    i_slint_core::OPERATING_SYSTEM_OVERRIDE
        .set(Some(i_slint_core::items::OperatingSystemType::Linux));

    let renderer = Rc::new(AnyrenderSlintRenderer::new_recording());
    i_slint_core::platform::set_platform(Box::new(ReplayBackend { renderer: renderer.clone() }))
        .map_err(|_| "platform already initialized")?;

    // Same deterministic font setup as the test driver.
    i_slint_backend_testing::configure_test_fonts();

    let source = std::fs::read_to_string(&case_path)?;
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let compiled =
        poll_once(compiler.build_from_source(source, case_path.clone())).expect("compile");
    if compiled.has_errors() {
        compiled.print_diagnostics();
        return Err("build error".into());
    }
    let def = compiled.components().last().expect("at least one exported component");
    let component = def.create()?;
    component.show()?;

    let scene = renderer.record()?;
    let size = component.window().size();

    eprintln!("Captured {} commands at {}x{}", scene.commands.len(), size.width, size.height);

    // Render the captured Scene through the production vello_cpu pipeline.
    let mut image_renderer = VelloCpuImageRenderer::new(size.width, size.height);
    let mut buf: Vec<u8> = Vec::with_capacity((size.width * size.height * 4) as usize);
    image_renderer.render_to_vec(
        |painter| {
            <_ as anyrender::PaintScene>::append_scene(painter, scene, kurbo::Affine::IDENTITY);
        },
        &mut buf,
    );

    // Buffer is RGBA8 premultiplied; convert to straight alpha for PNG.
    for px in buf.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255 / a).min(255)) as u8;
            px[1] = ((px[1] as u32 * 255 / a).min(255)) as u8;
            px[2] = ((px[2] as u32 * 255 / a).min(255)) as u8;
        }
    }

    let img =
        image::RgbaImage::from_raw(size.width, size.height, buf).expect("buffer size mismatch");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    img.save(&output_path)?;

    eprintln!("Wrote {}", output_path.display());

    let skia_ref = manifest_dir.join("references/skia").join(format!("{case_clean}.png"));
    if skia_ref.exists() {
        eprintln!("Skia reference for visual diff:\n  {}", skia_ref.display());
    }
    let software_ref = manifest_dir.join("references/software").join(format!("{case_clean}.png"));
    if software_ref.exists() {
        eprintln!("Software reference for visual diff:\n  {}", software_ref.display());
    }

    Ok(())
}
