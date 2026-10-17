// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::display::Presenter;
use crate::display::swdisplay::SoftwareBufferDisplay;
use crate::fullscreenwindowadapter::FullscreenWindowAdapter;
use i_slint_core::Property;
use i_slint_core::api::{LogicalPosition, LogicalSize};
use i_slint_core::cursor::MouseCursorInner;
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer, euclid};
use i_slint_core::items::BuiltInMouseCursor;
use i_slint_core::lengths::LogicalRect;
use i_slint_core::platform::{Platform, WindowEvent};
use i_slint_core::window::{WindowAdapter, WindowAdapterInternal};
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
        background: #304050;
        Rectangle { x: 25px; y: 20px; width: 60px; height: 45px; background: #c06030; }
    }
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
    let position = Box::pin(Property::new(None));
    let mut texture = SharedPixelBuffer::<Rgba8Pixel>::new(7, 9);
    texture.make_mut_slice().fill(Rgba8Pixel::new(255, 255, 255, 180));
    let custom = MouseCursorInner::CustomMouseCursor {
        image: Image::from_rgba8(texture),
        hotspot_x: 3,
        hotspot_y: 4,
    };
    let states = [
        (Some(LogicalPosition::new(10., 10.)), MouseCursorInner::default()),
        (Some(LogicalPosition::new(13., 12.)), MouseCursorInner::default()),
        (Some(LogicalPosition::new(40., 30.)), custom.clone()),
        (Some(LogicalPosition::new(0., 0.)), custom),
        (Some(LogicalPosition::new(79., 59.)), MouseCursorInner::default()),
        (Some(LogicalPosition::new(79., 59.)), MouseCursorInner::BuiltIn(BuiltInMouseCursor::None)),
        (Some(LogicalPosition::new(25., 25.)), MouseCursorInner::default()),
        (None, MouseCursorInner::default()),
    ];

    for scale in [1., 1.5, 2.] {
        scene.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
        let size = rotation.screen_size_to_rotated_window_size(SIZE);
        scene.window().dispatch_event(WindowEvent::Resized {
            size: LogicalSize::new(size.width as f32 / scale, size.height as f32 / scale),
        });
        let render_state = |(point, cursor): &(Option<LogicalPosition>, MouseCursorInner)| {
            position.as_ref().set(*point);
            adapter.set_mouse_cursor(cursor.clone());
            adapter.clone().render_if_needed(position.as_ref()).unwrap();
            display.pixels()
        };
        display.full_repaint.set(true);
        let expected: Vec<_> = states.iter().map(render_state).collect();
        if rotation == RenderingRotation::NoRotation
            && format == drm::buffer::DrmFourcc::Xrgb8888
            && !swapped
            && scale == 1.
            && let Some(directory) = std::env::var_os("SLINT_CURSOR_TEST_OUTPUT")
        {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            for (name, frame) in
                [("visible", &expected[0]), ("moved", &expected[6]), ("hidden", &expected[7])]
            {
                let mut ppm = format!("P6\n{} {}\n255\n", SIZE.width, SIZE.height).into_bytes();
                for row in frame.chunks_exact(row_bytes) {
                    for pixel in row[..SIZE.width as usize * 4].as_chunks::<4>().0 {
                        let value = u32::from_ne_bytes(*pixel);
                        ppm.extend_from_slice(&[
                            (value >> 16) as u8,
                            (value >> 8) as u8,
                            value as u8,
                        ]);
                    }
                }
                std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
            }
        }
        // A missing overlay must fail even when incremental and full frames agree.
        assert_ne!(expected[0], expected[7], "cursor was not drawn");
        display.full_repaint.set(false);
        display.initialized.set([false; 2]);
        adapter
            .renderer()
            .mark_dirty_region(LogicalRect::from_size(euclid::size2(160., 160.)).into());
        for (state, expected) in states.iter().zip(expected) {
            display.advance();
            let incremental = render_state(state);
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
