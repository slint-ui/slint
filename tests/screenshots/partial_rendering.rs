// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integration tests for partial rendering with transforms.
//!
//! Verifies that items under `transform-scale` and `transform-rotation` parents
//! are not incorrectly culled by the partial renderer when their local
//! coordinates are large but their screen-space coordinates (after the
//! transform) are within the viewport.
//!
//! All sub-tests run inside a single `#[test]` function because
//! `set_platform` can only be called once per process.

use i_slint_core::api::PhysicalSize;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext, Surface};
use slint_interpreter::ComponentHandle;

use std::cell::{Cell, RefCell};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

/// Pixel buffer surface that enables partial rendering.
///
/// Tracks render count so that `age` can be set to 0 (full repaint) on the
/// first render and 1 (partial repaint) on subsequent renders.
struct TestSurface {
    pixels: Rc<RefCell<Vec<u8>>>,
    render_count: Cell<u32>,
}

impl TestSurface {
    fn new() -> Self {
        Self { pixels: Rc::new(RefCell::new(Vec::new())), render_count: Cell::new(0) }
    }
}

impl Surface for TestSurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        _window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        _display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        _size: PhysicalSize,
        _requested_graphics_api: Option<i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError> {
        Err("TestSurface is created directly".into())
    }

    fn name(&self) -> &'static str {
        "test-partial-rendering"
    }

    fn resize_event(&self, _size: PhysicalSize) -> Result<(), PlatformError> {
        Ok(())
    }

    fn render(
        &self,
        _window: &i_slint_core::api::Window,
        size: PhysicalSize,
        render_callback: &dyn Fn(
            &i_slint_renderer_skia::skia_safe::Canvas,
            Option<&mut i_slint_renderer_skia::skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        _pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), PlatformError> {
        let Some(width) = NonZeroU32::new(size.width) else { return Ok(()) };
        let Some(height) = NonZeroU32::new(size.height) else { return Ok(()) };

        let byte_count = (width.get() as usize) * (height.get() as usize) * 4;
        let mut buf = self.pixels.borrow_mut();
        buf.resize(byte_count, 0);

        let mut skia_surface = i_slint_renderer_skia::skia_safe::surfaces::wrap_pixels(
            &i_slint_renderer_skia::skia_safe::ImageInfo::new(
                (width.get() as i32, height.get() as i32),
                i_slint_renderer_skia::skia_safe::ColorType::BGRA8888,
                i_slint_renderer_skia::skia_safe::AlphaType::Opaque,
                None,
            ),
            &mut buf,
            None,
            None,
        )
        .ok_or_else(|| PlatformError::from("Failed to wrap pixel buffer"))?;

        // age=0 on first render (full repaint), age=1 thereafter (partial).
        let count = self.render_count.get();
        let age = if count == 0 { 0 } else { 1 };
        self.render_count.set(count + 1);

        render_callback(skia_surface.canvas(), None, age);
        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        Ok(32)
    }

    fn use_partial_rendering(&self) -> bool {
        true
    }
}

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let surface = TestSurface::new();
        let pixels = surface.pixels.clone();
        let context = SkiaSharedContext::default();
        let renderer = SkiaRenderer::new_with_surface(&context, Box::new(surface));

        let adapter = Rc::new_cyclic(|self_weak| TestWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Cell::new(PhysicalSize::default()),
            renderer,
            pixels,
        });

        TEST_ADAPTER.with(|a| *a.borrow_mut() = Some(adapter.clone()));
        Ok(adapter)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

thread_local! {
    static TEST_ADAPTER: RefCell<Option<Rc<TestWindow>>> = const { RefCell::new(None) };
}

struct TestWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: SkiaRenderer,
    pixels: Rc<RefCell<Vec<u8>>>,
}

impl WindowAdapter for TestWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        let s = self.size.get();
        if s.width == 0 { PhysicalSize::new(300, 300) } else { s }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()));
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }
}

fn get_pixel_rgb(buf: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8) {
    let offset = ((y * width + x) * 4) as usize;
    // BGRA8888: [B, G, R, A]
    (buf[offset + 2], buf[offset + 1], buf[offset])
}

fn is_red(rgb: (u8, u8, u8)) -> bool {
    rgb.0 > 200 && rgb.1 < 50 && rgb.2 < 50
}

fn is_white(rgb: (u8, u8, u8)) -> bool {
    rgb.0 > 200 && rgb.1 > 200 && rgb.2 > 200
}

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    let future = std::pin::pin!(future);
    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

/// Compile a .slint source, create the component, show it, render once, and
/// return the adapter for pixel inspection.
fn render_test_component(
    source: &str,
) -> Result<(slint_interpreter::ComponentInstance, Rc<TestWindow>), Box<dyn std::error::Error>> {
    let compiler = slint_interpreter::Compiler::default();
    let compiled =
        poll_once(compiler.build_from_source(source.to_string(), "test.slint".into())).unwrap();

    if compiled.has_errors() {
        compiled.print_diagnostics();
        return Err("compilation failed".into());
    }

    let def = compiled.components().last().unwrap();
    let component = def.create()?;
    component.show()?;

    let adapter =
        TEST_ADAPTER.with(|a| a.borrow().clone()).expect("TestWindow adapter not found");

    adapter.renderer.render()?;
    Ok((component, adapter))
}

// ---------------------------------------------------------------------------
// Tests — all run inside a single #[test] because set_platform is once-only
// ---------------------------------------------------------------------------

#[test]
fn partial_rendering_with_transforms() -> Result<(), Box<dyn std::error::Error>> {
    i_slint_core::platform::set_platform(Box::new(TestPlatform))
        .expect("platform already initialized");

    test_scale()?;
    test_rotation()?;
    test_nested_scales()?;
    test_partial_repaint_with_scale()?;

    Ok(())
}

/// Items under a `transform-scale` parent at large local coordinates must not
/// be culled when their screen-space position (after scaling) is within the
/// viewport.
fn test_scale() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        export component Test inherits Window {
            width: 300px;
            height: 300px;
            background: white;

            Rectangle {
                x: 0px; y: 0px;
                width: 1000px; height: 1000px;
                transform-origin: { x: 0px, y: 0px };
                transform-scale-x: 0.5;
                transform-scale-y: 0.5;

                Rectangle {
                    x: 400px; y: 400px;
                    width: 100px; height: 100px;
                    background: red;
                }
            }
        }
    "#;

    let (_component, adapter) = render_test_component(source)?;
    let buf = adapter.pixels.borrow();

    // Scaled rect at local (400,400) size 100 → screen (200,200) size 50.
    let inside = get_pixel_rgb(&buf, 300, 220, 220);
    assert!(
        is_red(inside),
        "test_scale: pixel at (220,220) should be red, got {inside:?}"
    );

    let outside = get_pixel_rgb(&buf, 300, 260, 260);
    assert!(
        is_white(outside),
        "test_scale: pixel at (260,260) should be white, got {outside:?}"
    );

    Ok(())
}

/// Items under a `transform-rotation` parent must not be culled when their
/// screen-space position (after rotation) is within the viewport.
fn test_rotation() -> Result<(), Box<dyn std::error::Error>> {
    // 90° clockwise rotation around (150,150).
    // Local rect (250,150) size 50x50 → screen (100,250) size (50,50).
    let source = r#"
        export component Test inherits Window {
            width: 300px;
            height: 300px;
            background: white;

            Rectangle {
                x: 0px; y: 0px;
                width: 600px; height: 600px;
                transform-origin: { x: 150px, y: 150px };
                transform-rotation: 90deg;

                Rectangle {
                    x: 250px; y: 150px;
                    width: 50px; height: 50px;
                    background: red;
                }
            }
        }
    "#;

    let (_component, adapter) = render_test_component(source)?;
    let buf = adapter.pixels.borrow();

    // Screen rect at (100,250) size (50,50). Check center at (125, 275).
    let inside = get_pixel_rgb(&buf, 300, 125, 275);
    assert!(
        is_red(inside),
        "test_rotation: pixel at (125,275) should be red, got {inside:?}"
    );

    // Outside the rotated rect.
    let outside = get_pixel_rgb(&buf, 300, 80, 240);
    assert!(
        is_white(outside),
        "test_rotation: pixel at (80,240) should be white, got {outside:?}"
    );

    Ok(())
}

/// Nested scale transforms: scale(0.5) parent containing scale(0.5) child
/// with an item at large local coordinates. Verifies the transform stack
/// composes correctly across multiple save/restore levels.
fn test_nested_scales() -> Result<(), Box<dyn std::error::Error>> {
    // Outer: scale 0.5 from origin (0,0) — maps local → screen * 0.5.
    // Inner: scale 0.5 from origin (0,0) — maps local → parent * 0.5.
    // Combined: local * 0.25 in screen space.
    // Red rect at local (800, 800) size 200x200 → screen (200, 200) size (50, 50).
    let source = r#"
        export component Test inherits Window {
            width: 300px;
            height: 300px;
            background: white;

            Rectangle {
                x: 0px; y: 0px;
                width: 2000px; height: 2000px;
                transform-origin: { x: 0px, y: 0px };
                transform-scale-x: 0.5;
                transform-scale-y: 0.5;

                Rectangle {
                    x: 0px; y: 0px;
                    width: 2000px; height: 2000px;
                    transform-origin: { x: 0px, y: 0px };
                    transform-scale-x: 0.5;
                    transform-scale-y: 0.5;

                    Rectangle {
                        x: 800px; y: 800px;
                        width: 200px; height: 200px;
                        background: red;
                    }
                }
            }
        }
    "#;

    let (_component, adapter) = render_test_component(source)?;
    let buf = adapter.pixels.borrow();

    // Screen position: 800 * 0.25 = 200, size 200 * 0.25 = 50.
    let inside = get_pixel_rgb(&buf, 300, 220, 220);
    assert!(
        is_red(inside),
        "test_nested_scales: pixel at (220,220) should be red, got {inside:?}"
    );

    let outside = get_pixel_rgb(&buf, 300, 260, 260);
    assert!(
        is_white(outside),
        "test_nested_scales: pixel at (260,260) should be white, got {outside:?}"
    );

    Ok(())
}

/// After an initial render (age=0), changing a property and rendering again
/// (age=1) must still show the scaled item. This exercises the actual partial
/// rendering dirty-region path rather than just the full-repaint filter_item
/// path.
fn test_partial_repaint_with_scale() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        export component Test inherits Window {
            width: 300px;
            height: 300px;
            background: white;
            in-out property <int> dummy: 0;

            Rectangle {
                x: 0px; y: 0px;
                width: 1000px; height: 1000px;
                transform-origin: { x: 0px, y: 0px };
                transform-scale-x: 0.5;
                transform-scale-y: 0.5;

                Rectangle {
                    x: 400px; y: 400px;
                    width: 100px; height: 100px;
                    background: dummy == 0 ? red : blue;
                }
            }
        }
    "#;

    let (component, adapter) = render_test_component(source)?;

    // Frame 1 (age=0): full repaint. Red rectangle should be visible.
    {
        let buf = adapter.pixels.borrow();
        let px = get_pixel_rgb(&buf, 300, 220, 220);
        assert!(
            is_red(px),
            "test_partial_repaint frame 1: pixel at (220,220) should be red, got {px:?}"
        );
    }

    // Change a property to dirty the rectangle.
    component.set_property("dummy", slint_interpreter::Value::Number(1.0))?;

    // Frame 2 (age=1): partial repaint via PartialRenderer.
    adapter.renderer.render()?;

    {
        let buf = adapter.pixels.borrow();
        let (r, g, b) = get_pixel_rgb(&buf, 300, 220, 220);
        // Should now be blue (dummy changed to 1).
        assert!(
            b > 200 && r < 50 && g < 50,
            "test_partial_repaint frame 2: pixel at (220,220) should be blue after \
             property change, got rgb=({r}, {g}, {b}). If white, the partial renderer \
             culled the item during the age=1 repaint."
        );
    }

    Ok(())
}
