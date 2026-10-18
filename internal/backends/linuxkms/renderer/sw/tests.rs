// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::display::Presenter;
use crate::display::swdisplay::SoftwareBufferDisplay;
use crate::fullscreenwindowadapter::FullscreenWindowAdapter;
use i_slint_core::api::{LogicalPosition, LogicalSize};
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer, euclid};
use i_slint_core::lengths::LogicalRect;
use i_slint_core::platform::{Platform, WindowEvent};
use i_slint_core::window::WindowAdapter;
use slint::ComponentHandle;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const SIZE: PhysicalWindowSize = PhysicalWindowSize::new(160, 120);
const PADDING: usize = 16;

struct MemoryDisplay {
    buffers: RefCell<[Vec<u8>; 2]>,
    initialized: Cell<[bool; 2]>,
    index: Cell<usize>,
    format: drm::buffer::DrmFourcc,
    swapped: bool,
    full_repaint: Cell<bool>,
}

impl MemoryDisplay {
    fn bytes_per_pixel(&self) -> usize {
        if self.format == drm::buffer::DrmFourcc::Rgb565 { 2 } else { 4 }
    }

    fn pixels(&self) -> Vec<u8> {
        self.buffers.borrow()[self.index.get()].clone()
    }

    fn advance(&self) {
        if self.swapped {
            self.index.set(1 - self.index.get());
        }
    }
}

impl SoftwareBufferDisplay for MemoryDisplay {
    fn size(&self) -> (u32, u32) {
        (SIZE.width, SIZE.height)
    }

    fn map_back_buffer(
        &self,
        callback: &mut dyn FnMut(
            &mut [u8],
            u8,
            drm::buffer::DrmFourcc,
        ) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        let index = self.index.get();
        let mut initialized = self.initialized.get();
        let age = if self.full_repaint.get() || !initialized[index] {
            0
        } else if self.swapped {
            2
        } else {
            1
        };
        callback(&mut self.buffers.borrow_mut()[index], age, self.format)?;
        initialized[index] = true;
        self.initialized.set(initialized);
        Ok(())
    }

    fn is_write_combined_memory(&self) -> bool {
        false
    }

    fn as_presenter(self: Arc<Self>) -> Arc<dyn Presenter> {
        self
    }
}

impl Presenter for MemoryDisplay {
    fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

struct MemoryPlatform {
    adapter: Rc<FullscreenWindowAdapter>,
}

impl Platform for MemoryPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.adapter.clone())
    }
}

slint::slint! {
    export component CursorScene inherits Window {
        // One of the `Shape`s below.
        in property <int> shape;
        in property <image> custom-image;
        background: #304050;
        Rectangle { x: 25px; y: 20px; width: 60px; height: 45px; background: #c06030; }
        TouchArea {
            mouse-cursor: root.shape == 1 ? MouseCursor.custom(root.custom-image, 3, 4)
                : root.shape == 2 ? MouseCursor.none : MouseCursor.default;
        }
    }
}

/// The mouse cursor the scene asks for, as `CursorScene.shape`.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Default = 0,
    /// `CursorScene.custom-image` with the hotspot at (3, 4).
    Custom = 1,
    Hidden = 2,
}

/// Returns the byte offset in the frame buffer of the window pixel `(x, y)`.
///
/// This follows the documented direction of each rotation rather than the renderer's own
/// transform, so that the test catches a cursor drawn at a mirrored or transposed position.
fn buffer_offset(
    rotation: RenderingRotation,
    (x, y): (usize, usize),
    row_bytes: usize,
    bytes_per_pixel: usize,
) -> usize {
    let (width, height) = (SIZE.width as usize, SIZE.height as usize);
    let (screen_x, screen_y) = match rotation {
        RenderingRotation::NoRotation => (x, y),
        // 90° to the right: the window's left edge ends up at the top of the screen.
        RenderingRotation::Rotate90 => (width - 1 - y, x),
        RenderingRotation::Rotate180 => (width - 1 - x, height - 1 - y),
        // 90° to the left: the window's left edge ends up at the bottom of the screen.
        RenderingRotation::Rotate270 => (y, height - 1 - x),
    };
    screen_y * row_bytes + screen_x * bytes_per_pixel
}

fn cursor_frames(rotation: RenderingRotation, format: drm::buffer::DrmFourcc, swapped: bool) {
    let bpp = if format == drm::buffer::DrmFourcc::Rgb565 { 2 } else { 4 };
    let row_bytes = SIZE.width as usize * bpp + PADDING;
    #[expect(
        clippy::arc_with_non_send_sync,
        reason = "The display API uses Arc on the render thread."
    )]
    let display = Arc::new(MemoryDisplay {
        buffers: RefCell::new(std::array::from_fn(|_| vec![0; row_bytes * SIZE.height as usize])),
        initialized: Cell::new([false; 2]),
        index: Cell::new(0),
        format,
        swapped,
        full_repaint: Cell::new(true),
    });
    let renderer = SoftwareRendererAdapter {
        renderer: SoftwareRenderer::new(),
        display: display.clone(),
        presenter: display.clone(),
        size: SIZE,
    };
    let adapter = FullscreenWindowAdapter::new(Box::new(renderer), rotation).unwrap();
    i_slint_core::platform::set_platform(Box::new(MemoryPlatform { adapter: adapter.clone() }))
        .unwrap();
    let scene = CursorScene::new().unwrap();
    scene.show().unwrap();
    let mut texture = SharedPixelBuffer::<Rgba8Pixel>::new(7, 9);
    texture.make_mut_slice().fill(Rgba8Pixel::new(255, 255, 255, 180));
    scene.set_custom_image(Image::from_rgba8(texture));
    // `None` is a pointer that left the window. The shapes come from hovering the `TouchArea`,
    // so every position is inside the window, which is 60x80 or 80x60 at scale 2.
    // Every state must change what's drawn, or it doesn't schedule the frame it compares.
    let states = [
        (Some(LogicalPosition::new(10., 10.)), Shape::Default),
        (Some(LogicalPosition::new(13., 12.)), Shape::Default),
        (Some(LogicalPosition::new(40., 30.)), Shape::Custom),
        (Some(LogicalPosition::new(0., 0.)), Shape::Custom),
        (Some(LogicalPosition::new(59., 59.)), Shape::Default),
        (Some(LogicalPosition::new(59., 59.)), Shape::Hidden),
        (Some(LogicalPosition::new(25., 25.)), Shape::Default),
        (None, Shape::Default),
    ];

    for scale in [1., 1.5, 2.] {
        scene.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
        let size = rotation.screen_size_to_rotated_window_size(SIZE);
        scene.window().dispatch_event(WindowEvent::Resized {
            size: LogicalSize::new(size.width as f32 / scale, size.height as f32 / scale),
        });
        let render_state = |(position, shape): &(Option<LogicalPosition>, Shape)| {
            // The cursor shape follows the next pointer event.
            scene.set_shape(*shape as i32);
            scene.window().dispatch_event(match position {
                Some(position) => WindowEvent::PointerMoved { position: *position },
                None => WindowEvent::PointerExited,
            });
            adapter.clone().render_if_needed().unwrap();
            display.pixels()
        };
        display.full_repaint.set(true);
        let expected: Vec<_> = states.iter().map(render_state).collect();
        // A missing overlay must fail even when incremental and full frames agree.
        assert_ne!(expected[0], expected[7], "cursor was not drawn");
        // States 0 and 1 only differ by the pointer position, so this frame exists only if
        // the pointer movement scheduled it.
        assert_ne!(expected[0], expected[1], "pointer movement did not schedule a frame");
        // The incremental frames below are compared against frames drawn by the same code,
        // so check independently that the custom cursor of state 2 lands where it should.
        // It covers the logical rect (37, 26) 7x9: sample its center and a point to its right.
        let pixel = |frame: &[u8], logical_x: f32, logical_y: f32| {
            let window_pixel = ((logical_x * scale) as usize, (logical_y * scale) as usize);
            let offset = buffer_offset(rotation, window_pixel, row_bytes, bpp);
            frame[offset..offset + bpp].to_vec()
        };
        assert_ne!(
            pixel(&expected[2], 40.5, 30.5),
            pixel(&expected[7], 40.5, 30.5),
            "cursor missing at its position, scale {scale}"
        );
        assert_eq!(
            pixel(&expected[2], 47.5, 30.5),
            pixel(&expected[7], 47.5, 30.5),
            "cursor drawn outside its bounds, scale {scale}"
        );
        display.full_repaint.set(false);
        display.initialized.set([false; 2]);
        adapter
            .renderer()
            .mark_dirty_region(LogicalRect::from_size(euclid::size2(160., 160.)).into());
        for (state, expected) in states.iter().zip(expected) {
            display.advance();
            let incremental = render_state(state);
            // Not assert_eq!, which would print both frame buffers in full.
            assert!(incremental == expected, "cursor damage at {state:?}, scale {scale}");
            for row in incremental.chunks_exact(row_bytes) {
                assert!(
                    row[SIZE.width as usize * display.bytes_per_pixel()..].iter().all(|b| *b == 0)
                );
            }
        }
    }
}

#[test]
fn reused_xrgb_cursor_frames() {
    cursor_frames(RenderingRotation::NoRotation, drm::buffer::DrmFourcc::Xrgb8888, false);
}

#[test]
fn swapped_bgra_cursor_frames() {
    cursor_frames(RenderingRotation::NoRotation, drm::buffer::DrmFourcc::Bgra8888, true);
}

#[test]
fn rotated_90_rgb565_cursor_frames() {
    cursor_frames(RenderingRotation::Rotate90, drm::buffer::DrmFourcc::Rgb565, true);
}

#[test]
fn rotated_180_argb_cursor_frames() {
    cursor_frames(RenderingRotation::Rotate180, drm::buffer::DrmFourcc::Argb8888, true);
}

#[test]
fn rotated_270_xrgb_cursor_frames() {
    cursor_frames(RenderingRotation::Rotate270, drm::buffer::DrmFourcc::Xrgb8888, true);
}
