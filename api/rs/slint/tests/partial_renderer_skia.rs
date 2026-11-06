// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Tests of the partial renderer with the Skia renderer.

use i_slint_renderer_skia::SkiaRenderer;
use i_slint_renderer_skia::SkiaSharedContext;
use i_slint_renderer_skia::skia_safe;
use slint::platform::{PlatformError, WindowAdapter};
use slint::{PhysicalSize, SharedPixelBuffer};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

thread_local! {
    static SKIA_WINDOW: Rc<SkiaTestWindow> = SkiaTestWindow::new();
}

struct TestPlatform;
impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(SKIA_WINDOW.with(|x| x.clone()))
    }
}

const BYTES_PER_PIXEL: usize = 4;

struct SkiaTestWindow {
    window: slint::Window,
    renderer: SkiaRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<slint::PhysicalSize>,
    render_buffer: Rc<SkiaTestSoftwareBuffer>,
}

impl SkiaTestWindow {
    fn new() -> Rc<Self> {
        let render_buffer = Rc::new(SkiaTestSoftwareBuffer::default());
        let renderer = SkiaRenderer::new_with_surface(
            &SkiaSharedContext::default(),
            Box::new(i_slint_renderer_skia::software_surface::SoftwareSurface::from(
                render_buffer.clone(),
            )),
        );
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: slint::Window::new(w.clone()),
            renderer,
            needs_redraw: Default::default(),
            size: Default::default(),
            render_buffer,
        })
    }

    fn draw_if_needed(&self) -> bool {
        if self.needs_redraw.replace(false) {
            let _ = self.renderer.render().unwrap();
            true
        } else {
            false
        }
    }

    fn last_dirty_region_bounding_box_size(&self) -> Option<slint::LogicalSize> {
        self.render_buffer.last_dirty_region.borrow().as_ref().map(|r| {
            let size = r.bounding_rect().size;
            slint::LogicalSize::new(size.width as _, size.height as _)
        })
    }
    fn pixel(&self, x: u32, y: u32) -> slint::Rgba8Pixel {
        let pixels = self.render_buffer.pixels.borrow();
        let pixels = pixels.as_ref().unwrap();
        pixels.as_slice()[(y * pixels.width() + x) as usize]
    }
}

/// Creates a component with `create` and shows it in the Skia test window, after a first full frame.
fn show_in_skia_window<T: slint::ComponentHandle>(
    create: impl FnOnce() -> Result<T, PlatformError>,
    width: u32,
    height: u32,
) -> (T, Rc<SkiaTestWindow>) {
    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let window = SKIA_WINDOW.with(|w| w.clone());
    let ui = create().unwrap();
    window.set_size(slint::PhysicalSize::new(width, height).into());
    ui.show().unwrap();
    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize::new(width as f32, height as f32))
    );
    assert!(!window.draw_if_needed());
    (ui, window)
}

/// A color the renderer never writes, as it's not opaque.
const MARKER: slint::Rgba8Pixel = slint::Rgba8Pixel::new(1, 2, 3, 4);

/// Asserts that the next frame repaints exactly the pixels in `rects` (x, y, width, height),
/// like a full repaint, and that the frame after repaints nothing.
#[track_caller]
fn assert_skia_repaints(window: &SkiaTestWindow, rects: &[(u32, u32, u32, u32)]) {
    let pixels = || window.render_buffer.pixels.borrow().clone().unwrap();
    let before = pixels();
    window.render_buffer.pixels.borrow_mut().as_mut().unwrap().make_mut_slice().fill(MARKER);
    assert!(window.draw_if_needed());
    let partial = pixels();
    assert!(!window.draw_if_needed());

    *window.render_buffer.pixels.borrow_mut() = None;
    window.window.request_redraw();
    assert!(window.draw_if_needed());
    let full = pixels();

    let width = full.width();
    let pixels = partial.as_slice().iter().zip(full.as_slice()).zip(before.as_slice());
    for (i, ((partial, full), before)) in pixels.enumerate() {
        let (x, y) = (i as u32 % width, i as u32 / width);
        if rects
            .iter()
            .any(|&(rx, ry, rw, rh)| (rx..rx + rw).contains(&x) && (ry..ry + rh).contains(&y))
        {
            assert_eq!(partial, full, "({x}, {y}) should be repainted like in a full repaint");
        } else {
            assert_eq!(*partial, MARKER, "({x}, {y}) shouldn't be repainted");
            assert_eq!(before, full, "({x}, {y}) should be unchanged");
        }
    }
}

impl WindowAdapter for SkiaTestWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn set_size(&self, size: slint::WindowSize) {
        self.size.set(size.to_physical(1.));
        self.window
            .dispatch_event(slint::platform::WindowEvent::Resized { size: size.to_logical(1.) })
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
}

#[derive(Default)]
struct SkiaTestSoftwareBuffer {
    pixels: RefCell<Option<SharedPixelBuffer<slint::Rgba8Pixel>>>,
    last_dirty_region: RefCell<Option<i_slint_core::partial_renderer::DirtyRegion>>,
}

impl i_slint_renderer_skia::software_surface::RenderBuffer for SkiaTestSoftwareBuffer {
    fn with_buffer(
        &self,
        _window: &slint::Window,
        size: PhysicalSize,
        render_callback: &mut dyn FnMut(
            std::num::NonZeroU32,
            std::num::NonZeroU32,
            i_slint_renderer_skia::skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<i_slint_core::partial_renderer::DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)): Option<(std::num::NonZeroU32, std::num::NonZeroU32)> =
            size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        let mut shared_pixel_buffer = self.pixels.borrow_mut().take();

        if shared_pixel_buffer.as_ref().is_some_and(|existing_buffer| {
            existing_buffer.width() != width.get() || existing_buffer.height() != height.get()
        }) {
            shared_pixel_buffer = None;
        }

        let mut age = 1;
        let pixels = shared_pixel_buffer.get_or_insert_with(|| {
            age = 0;
            SharedPixelBuffer::new(width.get(), height.get())
        });

        let bytes = bytemuck::cast_slice_mut(pixels.make_mut_slice());
        *self.last_dirty_region.borrow_mut() =
            render_callback(width, height, skia_safe::ColorType::RGBA8888, age, bytes)?;

        *self.pixels.borrow_mut() = shared_pixel_buffer;

        Ok(())
    }
}

#[test]
fn rotated_image() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <angle> rotation <=> i.transform-rotation;
            in property <length> x-pos <=> i.x;
            background: black;
            i := Image {
                x: 50px;
                y: 50px;
                width: 50px;
                height: 150px;
                source: @image-url("../../../logo/slint-logo-full-dark.png");
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    ui.set_x_pos(51.);

    assert_skia_repaints(&window, &[(50, 50, 51, 150)]);

    ui.set_rotation(90.);

    // The rotated rect starts at y = 99.99999, which rounds out to row 99.
    assert_skia_repaints(&window, &[(51, 50, 50, 150), (1, 99, 150, 51)]);
}

#[test]
fn shadow_redraw_beyond_geometry() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <length> x-pos: 10px;
            Rectangle {
                x: root.x-pos;
                y: 10px;
                width: 20px;
                height: 20px;
                background: white;
                drop-shadow-blur: 5px;
                drop-shadow-offset-x: 15px;
                drop-shadow-offset-y: 5px;
                drop-shadow-color: red;
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    ui.set_x_pos(20.);

    let shadow_size = /* rect size */ 20 + 2 * /* blur */ 5;
    let old_shadow_x = /* rect x */ 10 + /* shadow offset */ 15 - /* blur */ 5;
    let shadow_y = /* rect y */ 10 + /* shadow offset */ 5 - /* blur */ 5;
    let new_shadow_x = old_shadow_x + /* move */ 10;

    assert_skia_repaints(
        &window,
        &[
            (10, 10, 20, 20),
            (old_shadow_x, shadow_y, shadow_size, shadow_size),
            (new_shadow_x, shadow_y, shadow_size, shadow_size),
        ],
    );
}

#[test]
fn backdrop_blur_redraws_as_a_whole() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <length> x-pos: 10px;
            Rectangle {
                x: root.x-pos;
                y: 20px;
                width: 10px;
                height: 10px;
                background: red;
            }
            Rectangle {
                x: 50px;
                y: 10px;
                width: 100px;
                height: 40px;
                background: #fff4;
                backdrop-blur: 4px;
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    // Moving far from the blur only repaints the old and new position.
    ui.set_x_pos(20.);
    assert_skia_repaints(&window, &[(10, 20, 20, 10)]);

    // Moving next to the blur, within the 3 * 4px it samples beyond its edges, repaints the
    // whole blur and that margin (clipped to the window at the top).
    ui.set_x_pos(30.);
    assert_skia_repaints(&window, &[(20, 20, 20, 10), (38, 0, 124, 62)]);
}

#[test]
fn backdrop_blur_opacity_over_translucent_backdrop() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            background: transparent;
            Rectangle {
                background: #ffffff40;
            }
            Rectangle {
                x: 50px;
                y: 50px;
                width: 100px;
                height: 40px;
                backdrop-blur: 4px;
                opacity: 0.5;
            }
        }
    }

    let (_ui, window) = show_in_skia_window(Ui::new, 250, 250);

    // Blurring and fading a uniform backdrop leaves it as it is, inside and outside the blur.
    let expected = slint::Rgba8Pixel { r: 64, g: 64, b: 64, a: 64 };
    for (x, y) in [(10, 10), (100, 70), (200, 200)] {
        let pixel = window.pixel(x, y);
        for (channel, expected) in [
            (pixel.r, expected.r),
            (pixel.g, expected.g),
            (pixel.b, expected.b),
            (pixel.a, expected.a),
        ] {
            assert!(channel.abs_diff(expected) <= 1, "pixel at {x},{y} is {pixel:?}");
        }
    }
}

#[test]
fn backdrop_blur_redraws_as_a_whole_for_a_removed_item() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> show: true;
            Rectangle {
                x: 50px;
                y: 50px;
                width: 100px;
                height: 40px;
                background: #fff4;
                backdrop-blur: 4px;
            }
            if root.show: Rectangle {
                x: 140px;
                y: 60px;
                width: 30px;
                height: 10px;
                background: red;
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    // The removed rectangle overlaps the blur, so the whole blur and its 3 * 4px margin repaint.
    ui.set_show(false);
    assert_skia_repaints(&window, &[(38, 38, 124, 64), (140, 60, 30, 10)]);
}

#[test]
fn backdrop_blur_redraws_for_changes_outside_its_clip() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <length> x-pos: 30px;
            Rectangle {
                x: root.x-pos;
                y: 60px;
                width: 10px;
                height: 10px;
                background: red;
            }
            Rectangle {
                x: 50px;
                y: 50px;
                width: 100px;
                height: 100px;
                clip: true;
                Rectangle {
                    x: 0px;
                    y: 0px;
                    width: 100px;
                    height: 40px;
                    background: #fff4;
                    backdrop-blur: 4px;
                }
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    // The red rectangle is outside the clip, but within the 3 * 4px the blur samples beyond it.
    // Everything the blur samples is repainted, including the parts outside of the clip,
    // so that the blur doesn't read the previous frame's pixels there.
    ui.set_x_pos(35.);
    assert_skia_repaints(&window, &[(30, 60, 15, 10), (38, 38, 124, 64)]);
}

#[test]
fn text_alignment() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <color> c: green;
            Text {
                x: 10px;
                y: 10px;
                width: 200px;
                height: 50px;
                text: "Ok";
                color: c;
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 250, 250);

    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));

    assert_skia_repaints(&window, &[(10, 10, 200, 50)]);
}

/// Items under a `transform-scale` parent at large local coordinates must not
/// be culled when their screen-space position (after scaling) is within the
/// viewport.
#[test]
fn partial_rendering_does_not_cull_scaled_items() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            width: 300px;
            height: 300px;
            background: white;
            in property <float> scale-val: 0.5;

            Rectangle {
                x: 0px; y: 0px;
                width: 1000px; height: 1000px;
                transform-origin: { x: 0px, y: 0px };
                transform-scale-x: scale-val;
                transform-scale-y: scale-val;

                Rectangle {
                    x: 400px; y: 400px;
                    width: 100px; height: 100px;
                    background: red;
                }
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 300, 300);

    // Check that the scaled rectangle is visible via pixel data.
    // At scale 0.5 from origin (0,0), rect at local (400,400) size 100
    // → screen (200,200) size 50.
    {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        {
            let data = buf.as_bytes();
            // RGBA8888, pixel at (220, 220)
            let offset = ((220 * 300 + 220) * 4) as usize;
            let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
            assert!(
                r > 200 && g < 50 && b < 50,
                "Frame 1: pixel at (220,220) should be red, got rgb=({r},{g},{b})"
            );
        }
    }

    assert!(!window.draw_if_needed());

    // Change scale to trigger a partial repaint.
    ui.set_scale_val(0.5001);

    // Frame 2: partial repaint. The item must not be culled.
    assert!(window.draw_if_needed());

    {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        {
            let data = buf.as_bytes();
            let offset = ((220 * 300 + 220) * 4) as usize;
            let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
            assert!(
                r > 200 && g < 50 && b < 50,
                "Frame 2: pixel at (220,220) should still be red after partial repaint, \
                 got rgb=({r},{g},{b})"
            );
        }
    }
}

/// Items under a `transform-rotation` parent must not be culled when their
/// screen-space position (after rotation) is within the viewport.
#[test]
fn partial_rendering_does_not_cull_rotated_items() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            width: 300px;
            height: 300px;
            background: white;
            in property <angle> rot: 90deg;

            Rectangle {
                x: 0px; y: 0px;
                width: 600px; height: 600px;
                transform-origin: { x: 150px, y: 150px };
                transform-rotation: rot;

                Rectangle {
                    x: 250px; y: 150px;
                    width: 50px; height: 50px;
                    background: red;
                }
            }
        }
    }

    let (_ui, window) = show_in_skia_window(Ui::new, 300, 300);

    // 90° clockwise rotation around (150,150):
    // local (250,150) → screen (100,250). Check center at (125, 275).
    {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        {
            let data = buf.as_bytes();
            let offset = ((275 * 300 + 125) * 4) as usize;
            let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
            assert!(
                r > 200 && g < 50 && b < 50,
                "Rotated rectangle should be visible at (125,275), got rgb=({r},{g},{b})"
            );
        }
    }
}

/// Nested scale transforms must compose correctly across save/restore.
#[test]
fn partial_rendering_nested_scales() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
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
    }

    let (_ui, window) = show_in_skia_window(Ui::new, 300, 300);

    // Combined scale 0.25: local (800,800) size 200 → screen (200,200) size 50.
    {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        {
            let data = buf.as_bytes();
            // Inside at (220,220)
            let offset = ((220 * 300 + 220) * 4) as usize;
            let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
            assert!(
                r > 200 && g < 50 && b < 50,
                "Nested scale: pixel at (220,220) should be red, got rgb=({r},{g},{b})"
            );
            // Outside at (260,260)
            let offset2 = ((260 * 300 + 260) * 4) as usize;
            let (r2, g2, b2) = (data[offset2], data[offset2 + 1], data[offset2 + 2]);
            assert!(
                r2 > 200 && g2 > 200 && b2 > 200,
                "Nested scale: pixel at (260,260) should be white, got rgb=({r2},{g2},{b2})"
            );
        }
    }
}

/// Regression test for https://github.com/slint-ui/slint/issues/11431.
///
/// A cached Layer (`cache-rendering-hint: true`) that starts zero-sized must
/// re-render when its size later becomes non-zero. Without tracking the
/// bounds-closure's dependencies on the zero-size path, the cache stores
/// `None` with an empty dependency tracker and never reruns, so the layer
/// stays invisible even after its size grows.
#[test]
fn layer_visible_after_becoming_non_zero_sized() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            width: 32px;
            height: 32px;
            background: white;
            in-out property <length> content-height: 0px;
            Rectangle {
                cache-rendering-hint: true;
                x: 4px;
                y: 4px;
                width: 24px;
                height: root.content-height;
                background: red;
            }
        }
    }

    // Frame 1: the layer is 0-height, so the cache update closure returns
    // None. The fix re-invokes the bounds closure with tracking so the
    // dependency on `content-height` gets registered.
    let (ui, window) = show_in_skia_window(Ui::new, 32, 32);

    ui.set_content_height(24.);

    // Frame 2: the tracked dependency is now dirty, the layer cache reruns,
    // and the red rectangle is drawn. Without the fix the cache stays on
    // the stale None and the pixel below remains white.
    assert!(window.draw_if_needed());

    let pixels = window.render_buffer.pixels.borrow();
    let buf = pixels.as_ref().expect("render buffer should contain pixels");
    let data = buf.as_bytes();
    let offset = ((16 * 32 + 16) * 4) as usize;
    let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
    assert!(
        r > 200 && g < 50 && b < 50,
        "Layer pixel at (16,16) should be red after content-height grew, got rgb=({r},{g},{b})"
    );
}

#[test]
fn layer_rendered_at_correct_position() {
    // Regression test for #11763: render_layer's compute_bounds used
    // item_rc.geometry() (parent coordinates) instead of a local-coords
    // rect. Inside a Flickable the parent offset is the content position
    // (large x), while the clip is in item-local coords. The union mixes
    // coordinate systems, producing a wrong layer origin when the item is
    // partially scrolled out of the visible area. This makes the layer
    // texture miss the left portion of the item.
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            width: 64px;
            height: 64px;
            background: white;
            in-out property <length> vpx: 0px;
            Flickable {
                width: 64px;
                height: 64px;
                content-width: 200px;
                content-x <=> root.vpx;
                interactive: false;

                Rectangle {
                    cache-rendering-hint: true;
                    x: 100px;
                    y: 0px;
                    width: 40px;
                    height: 40px;
                    Rectangle {
                        background: red;
                        width: 100%;
                        height: 100%;
                    }
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let window = SKIA_WINDOW.with(|w| w.clone());
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(64, 64).into());
    ui.show().unwrap();

    // Frame 1: item partially scrolled off the left edge of the Flickable.
    // content-x = -120 → visible range 120..184. Item at 100..140.
    // Item left edge (100) is 20px left of visible start (120).
    // Only the right 20px are visible in the Flickable.
    // The layer cache is created with the clip starting at x=20 in local
    // coords. With the bug, geometry (100,..) makes the union start at 20
    // instead of 0, so only the right 20px are captured.
    ui.set_vpx(-120.);
    assert!(window.draw_if_needed());

    // Frame 2: scroll so the item is fully visible. Do NOT change any
    // child property, so the layer cache stays valid and reuses the stale
    // texture from frame 1. With the bug, the stale texture has origin
    // (20, 0) instead of (0, 0), so 20px on the left are missing.
    window.render_buffer.pixels.borrow_mut().take(); // force full redraw
    ui.set_vpx(-60.);
    assert!(window.draw_if_needed());

    let pixels = window.render_buffer.pixels.borrow();
    let buf = pixels.as_ref().expect("render buffer should contain pixels");
    let data = buf.as_bytes();
    let stride = 64;

    // Item at vpx=-60: window x = 100 + (-60) = 40. Spans 40..80, clipped
    // at 64, so visible 40..63.
    // With the bug: stale cache has origin=(20,0), size=(20,40). Drawn at
    // (40+20, 0) = (60, 0), only 4px visible (60..63). Gap at 40..59.
    // With the fix: cache has origin=(0,0), size=(40,40). Drawn at (40, 0),
    // visible 40..63. Correct.
    // The item should start at window x=40.
    let off = ((5 * stride + 40) * 4) as usize;
    let (r, g, b) = (data[off], data[off + 1], data[off + 2]);
    assert!(
        r > 200 && g < 50 && b < 50,
        "Pixel at (40,5) should be red (left edge of cached layer), got rgb=({r},{g},{b})"
    );
}

#[test]
fn partial_rendering_popup_position_size_change() {
    slint::slint! {
        #[edition = "1.19"]
        global MyProperty {
            in-out property<length> popup-x: 0px;
            in-out property<length> popup-y: 0px;
            in-out property<length> popup-width: 100px;
            in-out property<length> popup-height: 200px;
        }

        export component Ui inherits Window {
            width: 600px;
            height: 600px;
            background: red;

            TouchArea {
                property<bool> was-clicked: false;
                clicked => {
                    if !was-clicked {
                        was-clicked = true;
                        show_popup();
                    }
                }
            }

            callback show_popup();
            show_popup() => {
                popup.show();
            }

            callback change_popup();
            change_popup() => {
                debug("Changing popup properties");
                MyProperty.popup-x = 10px;
                MyProperty.popup-y = 20px;
                MyProperty.popup-width = 150px;
                MyProperty.popup-height = 30px;
            }

            popup:= PopupWindow {
                x: MyProperty.popup-x;
                y: MyProperty.popup-y;
                width: MyProperty.popup-width;
                height: MyProperty.popup-height;
                close-policy: PopupClosePolicy.no-auto-close;

                TouchArea {
                    clicked => {
                        change_popup();
                    }
                }

                changed width => {
                    debug("Width changed");
                }

                Rectangle {
                    background: blue;
                }
            }
        }
    }

    const WINDOW_WIDTH: usize = 600;
    const WINDOW_HEIGHT: usize = 600;
    const POPUP_WIDTH: usize = 100;
    const POPUP_HEIGHT: usize = 200;
    const RGB_COLOR_WINDOW: (u8, u8, u8) = (255, 0, 0);
    const RGB_COLOR_POPUP: (u8, u8, u8) = (0, 0, 255);

    let (ui, window) = show_in_skia_window(Ui::new, WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);

    let get_pixel_values = |pixel_index: usize| {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        let data = buf.as_bytes();
        let offset = pixel_index * BYTES_PER_PIXEL;
        (data[offset], data[offset + 1], data[offset + 2])
    };

    // For debugging. Dump pixels
    let _dump_pixels = || {
        for v_pixel in 0..WINDOW_HEIGHT {
            for h_pixel in 0..WINDOW_WIDTH {
                let pixel_idx = WINDOW_WIDTH * v_pixel + h_pixel;
                let rgb = get_pixel_values(pixel_idx);
                // print!("{r:02x}{g:02x}{b:02x} ", r = rgb.0, g = rgb.1, b = rgb.2);
                if rgb.0 > 0 {
                    print!("r");
                } else if rgb.2 > 0 {
                    print!("b");
                } else {
                    print!("g")
                }
            }
            print!("\n");
        }
    };

    {
        let pixels = window.render_buffer.pixels.borrow();
        let buf = pixels.as_ref().expect("render buffer should contain pixels");
        assert_eq!(buf.as_bytes().len(), WINDOW_WIDTH * WINDOW_HEIGHT * BYTES_PER_PIXEL);
        for pixel_idx in 0..(WINDOW_WIDTH * WINDOW_HEIGHT) {
            let rgb = get_pixel_values(pixel_idx);
            assert_eq!(rgb, RGB_COLOR_WINDOW, "Wrong color at pixel index pixel_idx");
        }
    }

    ui.invoke_show_popup();
    assert!(window.draw_if_needed());

    {
        for v_pixel in 0..WINDOW_HEIGHT {
            for h_pixel in 0..WINDOW_WIDTH {
                let pixel_idx = WINDOW_WIDTH * v_pixel + h_pixel;

                if h_pixel < POPUP_WIDTH && v_pixel < POPUP_HEIGHT {
                    let rgb = get_pixel_values(pixel_idx);
                    assert_eq!(rgb, RGB_COLOR_POPUP, "Wrong color at pixel ({h_pixel}, {v_pixel})");
                } else {
                    let rgb = get_pixel_values(pixel_idx);
                    assert_eq!(
                        rgb, RGB_COLOR_WINDOW,
                        "Wrong color at pixel ({h_pixel}, {v_pixel})"
                    );
                }
            }
        }
    }

    ui.invoke_change_popup();
    // The popup properties change trigger a tracker with a timer we have to process before the next draw.
    slint::platform::update_timers_and_animations();

    assert!(window.draw_if_needed());

    {
        // New popup properties
        const POPUP_POS_X: usize = 10;
        const POPUP_POS_Y: usize = 20;
        const POPUP_WIDTH: usize = 150;
        const POPUP_HEIGHT: usize = 30;
        for v_pixel in 0..WINDOW_HEIGHT {
            for h_pixel in 0..WINDOW_WIDTH {
                let pixel_idx = WINDOW_WIDTH * v_pixel + h_pixel;

                if h_pixel >= POPUP_POS_X
                    && h_pixel < POPUP_POS_X + POPUP_WIDTH
                    && v_pixel >= POPUP_POS_Y
                    && v_pixel < POPUP_POS_Y + POPUP_HEIGHT
                {
                    let rgb = get_pixel_values(pixel_idx);
                    assert_eq!(
                        rgb, RGB_COLOR_POPUP,
                        "Wrong color at pixel ({h_pixel}, {v_pixel}). Expected popup color."
                    );
                } else {
                    let rgb = get_pixel_values(pixel_idx);
                    assert_eq!(
                        rgb, RGB_COLOR_WINDOW,
                        "Wrong color at pixel ({h_pixel}, {v_pixel}). Expected window color"
                    );
                }
            }
        }
    }
}
const GREEN: slint::Color = slint::Color::from_rgb_u8(0, 255, 0);

#[test]
/// #8495
fn nested_opacity_layers() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property outer-opacity <=> outer.opacity;
            in property mid-opacity <=> mid.opacity;
            in property deep-opacity <=> deep.opacity;
            in property deep-x <=> deep.x;
            in property outer-child-color <=> outer-child.background;
            in property mid-child-color <=> mid-child.background;
            in property deep-child-color <=> deep-child.background;
            in property overlap-color <=> overlap.background;
            in property crossing-color <=> crossing.background;
            in property outside-color <=> outside.background;
            background: white;
            // Window coordinates are in the comments.
            outer := Rectangle {
                // 10,10 100x100
                x: 10px; y: 10px; width: 100px; height: 100px; background: red;
                opacity: 0.8;
                // 15,15 20x10
                outer-child := Rectangle { x: 5px; y: 5px; width: 20px; height: 10px; background: black; }
                mid := Rectangle {
                    // 40,40 60x60
                    x: 30px; y: 30px; width: 60px; height: 60px; background: blue;
                    opacity: 0.8;
                    // 45,45 20x10
                    mid-child := Rectangle { x: 5px; y: 5px; width: 20px; height: 10px; background: black; }
                    deep := Rectangle {
                        // 70,70 25x25
                        x: 30px; y: 30px; width: 25px; height: 25px; background: green;
                        opacity: 0.8;
                        // 75,75 10x10
                        deep-child := Rectangle { x: 5px; y: 5px; width: 10px; height: 10px; background: black; }
                    }
                }
            }
            // Overlaps the corner of `outer` only.
            overlap := Rectangle { x: 0px; y: 0px; width: 15px; height: 15px; background: yellow; }
            // Crosses the corner of all three layers, but no child.
            crossing := Rectangle { x: 60px; y: 60px; width: 12px; height: 12px; background: yellow; }
            outside := Rectangle { x: 130px; y: 10px; width: 20px; height: 20px; background: yellow; }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 180, 260);

    ui.set_overlap_color(GREEN.into());
    assert_skia_repaints(&window, &[(0, 0, 15, 15)]);
    ui.set_crossing_color(GREEN.into());
    assert_skia_repaints(&window, &[(60, 60, 12, 12)]);
    ui.set_deep_child_color(GREEN.into());
    assert_skia_repaints(&window, &[(75, 75, 10, 10)]);
    ui.set_mid_child_color(GREEN.into());
    assert_skia_repaints(&window, &[(45, 45, 20, 10)]);
    ui.set_outer_child_color(GREEN.into());
    assert_skia_repaints(&window, &[(15, 15, 20, 10)]);
    ui.set_outside_color(GREEN.into());
    assert_skia_repaints(&window, &[(130, 10, 20, 20)]);

    ui.set_deep_opacity(0.5);
    assert_skia_repaints(&window, &[(70, 70, 25, 25)]);
    ui.set_deep_child_color(slint::Color::default().into());
    assert_skia_repaints(&window, &[(75, 75, 10, 10)]);
    ui.set_mid_opacity(0.5);
    assert_skia_repaints(&window, &[(40, 40, 60, 60)]);
    ui.set_crossing_color(slint::Color::default().into());
    assert_skia_repaints(&window, &[(60, 60, 12, 12)]);
    ui.set_deep_child_color(GREEN.into());
    assert_skia_repaints(&window, &[(75, 75, 10, 10)]);
    ui.set_outer_opacity(0.5);
    assert_skia_repaints(&window, &[(10, 10, 100, 100)]);
    ui.set_mid_child_color(slint::Color::default().into());
    assert_skia_repaints(&window, &[(45, 45, 20, 10)]);

    ui.set_deep_x(20.);
    assert_skia_repaints(&window, &[(60, 70, 35, 25)]);
    ui.set_deep_child_color(slint::Color::default().into());
    assert_skia_repaints(&window, &[(65, 75, 10, 10)]);
}

#[test]
fn cached_layer_between_opacity_layers() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property outer-child-color <=> outer-child.background;
            in property deep-child-color <=> deep-child.background;
            in property crossing-color <=> crossing.background;
            background: white;
            Rectangle {
                // 10,10 100x100
                x: 10px; y: 10px; width: 100px; height: 100px; background: red;
                opacity: 0.8;
                // 15,15 20x10
                outer-child := Rectangle { x: 5px; y: 5px; width: 20px; height: 10px; background: black; }
                Rectangle {
                    // 40,40 60x60
                    x: 30px; y: 30px; width: 60px; height: 60px; background: blue;
                    cache-rendering-hint: true;
                    Rectangle {
                        // 70,70 25x25
                        x: 30px; y: 30px; width: 25px; height: 25px; background: green;
                        opacity: 0.8;
                        // 75,75 10x10
                        deep-child := Rectangle { x: 5px; y: 5px; width: 10px; height: 10px; background: black; }
                    }
                }
            }
            // Crosses the corner of the three layers, but no child.
            crossing := Rectangle { x: 60px; y: 60px; width: 12px; height: 12px; background: yellow; }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 180, 260);

    for color in [GREEN, slint::Color::default()] {
        ui.set_crossing_color(color.into());
        assert_skia_repaints(&window, &[(60, 60, 12, 12)]);
        ui.set_deep_child_color(color.into());
        assert_skia_repaints(&window, &[(40, 40, 60, 60)]);
        ui.set_outer_child_color(color.into());
        assert_skia_repaints(&window, &[(15, 15, 20, 10)]);
    }
}

#[test]
fn opacity_layer_child_change_after_scrolling_into_view() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property inner-color <=> inner.background;
            in-out property content-y <=> flickable.content-y;
            background: white;
            flickable := Flickable {
                x: 10px; y: 10px; width: 60px; height: 40px;
                content-height: 200px;
                Rectangle {
                    width: 60px; height: 200px; background: red;
                    opacity: 0.5;
                    inner := Rectangle { x: 0px; y: 100px; width: 20px; height: 10px; background: blue; }
                }
            }
        }
    }

    let (ui, window) = show_in_skia_window(Ui::new, 180, 260);

    ui.set_content_y(-90.);
    assert_skia_repaints(&window, &[(10, 10, 60, 40)]);
    ui.set_inner_color(GREEN.into());
    assert_skia_repaints(&window, &[(10, 20, 20, 10)]);
}
