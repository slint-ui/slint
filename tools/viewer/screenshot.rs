// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{LogicalSize, PhysicalSize, Window, WindowSize};
use i_slint_core::platform::{Platform, PlatformError, WindowEvent};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::{WindowAdapter, WindowProperties};
use slint_interpreter::ComponentHandle;
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use crate::{
    Cli, Error, Result, extract_component, init_compiler, poll_ready, reject_non_window_component,
    setup_instance,
};

/// Build the best headless renderer compiled into the viewer. Skia's software
/// rasterizer is preferred when available; otherwise we fall back to Slint's
/// own software renderer.
#[cfg(all(
    any(
        feature = "renderer-skia",
        feature = "renderer-skia-opengl",
        feature = "renderer-skia-vulkan",
    ),
    not(target_os = "android"),
))]
fn create_renderer() -> Box<dyn Renderer> {
    Box::new(i_slint_renderer_skia::SkiaRenderer::default_software(
        &i_slint_renderer_skia::SkiaSharedContext::default(),
    ))
}

#[cfg(not(all(
    any(
        feature = "renderer-skia",
        feature = "renderer-skia-opengl",
        feature = "renderer-skia-vulkan",
    ),
    not(target_os = "android"),
)))]
fn create_renderer() -> Box<dyn Renderer> {
    Box::new(i_slint_renderer_software::SoftwareRenderer::new())
}

pub fn take_screenshot(args: &Cli) -> Result<()> {
    let output = args.screenshot.as_deref().expect("--screenshot was set");

    let size = args.size.as_deref().map(parse_size).transpose()?;

    // If the user didn't pick a backend explicitly, install our own headless software
    // backend so the screenshot does not require a windowing system.
    if args.backend.is_none() {
        i_slint_core::platform::set_platform(Box::new(ScreenshotPlatform { size }))
            .map_err(|e| Error::from(format!("Failed to initialize headless backend: {e}")))?;
    }

    let compiler = init_compiler(args);
    let result = poll_ready(compiler.build_from_path(args.path()));
    result.print_diagnostics();
    if result.has_errors() {
        std::process::exit(1);
    }
    let Some(c) = extract_component(&result, args) else {
        std::process::exit(1);
    };
    reject_non_window_component(&c);

    let component = c.create()?;
    setup_instance(&component, &args.on, args.load_data.as_deref())?;

    component.show()?;

    let buffer = component.window().take_snapshot()?;

    let format = image::ImageFormat::from_path(output).unwrap_or(image::ImageFormat::Png);
    // Some formats (such as JPEG) don't support an alpha channel; drop it when needed.
    let (pixels, color) = if format.writing_enabled() && format != image::ImageFormat::Jpeg {
        (buffer.as_bytes().to_vec(), image::ExtendedColorType::Rgba8)
    } else {
        let rgb = buffer.as_bytes().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        (rgb, image::ExtendedColorType::Rgb8)
    };

    if output == Path::new("-") {
        use std::io::Write;
        let mut encoded = std::io::Cursor::new(Vec::<u8>::new());
        image::write_buffer_with_format(
            &mut encoded,
            &pixels,
            buffer.width(),
            buffer.height(),
            color,
            format,
        )?;
        std::io::stdout().lock().write_all(&encoded.into_inner())?;
    } else {
        image::save_buffer_with_format(
            output,
            &pixels,
            buffer.width(),
            buffer.height(),
            color,
            format,
        )?;
    }

    Ok(())
}

/// Parse a `WIDTHxHEIGHT` size argument into a logical size.
fn parse_size(s: &str) -> Result<LogicalSize> {
    let invalid = || Error::from(format!("Invalid --size '{s}', expected WIDTHxHEIGHT"));
    let (w, h) = s.split_once(['x', 'X']).ok_or_else(invalid)?;
    let (w, h) =
        (w.trim().parse().map_err(|_| invalid())?, h.trim().parse().map_err(|_| invalid())?);
    Ok(LogicalSize::new(w, h))
}

struct ScreenshotPlatform {
    size: Option<LogicalSize>,
}

impl Platform for ScreenshotPlatform {
    fn create_window_adapter(&self) -> std::result::Result<Rc<dyn WindowAdapter>, PlatformError> {
        let adapter = Rc::new_cyclic(|self_weak| ScreenshotWindow {
            window: Window::new(self_weak.clone() as _),
            size: Cell::default(),
            renderer: create_renderer(),
        });
        if let Some(scale_factor) =
            std::env::var("SLINT_SCALE_FACTOR").ok().and_then(|sf| sf.parse().ok())
        {
            adapter.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
        }
        // Force the requested size; otherwise the component's preferred size is used.
        if let Some(size) = self.size {
            adapter.set_size(WindowSize::Logical(size));
        }
        Ok(adapter)
    }
}

struct ScreenshotWindow {
    window: Window,
    size: Cell<PhysicalSize>,
    renderer: Box<dyn Renderer>,
}

impl WindowAdapter for ScreenshotWindow {
    fn window(&self) -> &Window {
        &self.window
    }
    fn renderer(&self) -> &dyn Renderer {
        &*self.renderer
    }
    fn size(&self) -> PhysicalSize {
        self.size.get()
    }
    fn set_size(&self, size: WindowSize) {
        let sf = self.window.scale_factor();
        self.size.set(size.to_physical(sf));
        self.window.dispatch_event(WindowEvent::Resized { size: size.to_logical(sf) });
    }
    fn update_window_properties(&self, properties: WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let preferred = properties.layout_constraints().preferred;
            let sf = self.window.scale_factor();
            self.size.set(preferred.to_physical(sf));
            self.window.dispatch_event(WindowEvent::Resized { size: preferred });
        }
    }
}
