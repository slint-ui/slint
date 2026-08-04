// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::Rgb8Pixel;
use slint::platform::software_renderer::{
    DirtyRegionAlignment, MinimalSoftwareWindow, PhysicalRegion, RenderingRotation,
    RepaintBufferType,
};
use slint::platform::{PlatformError, WindowAdapter};

// Window size; with Rotate90 the buffer is HEIGHT pixels wide and WIDTH lines tall.
const WIDTH: usize = 16;
const HEIGHT: usize = 8;

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

// The 4×2 alignment applies to the panel (buffer) axes, which are the window axes swapped.
// Using different values per axis catches an accidental axis swap in the implementation.
fn assert_aligned(region: &PhysicalRegion) {
    for (position, size) in region.iter() {
        assert_eq!(position.x % 4, 0);
        assert_eq!(position.y % 2, 0);
        assert_eq!(size.width % 4, 0);
        assert_eq!(size.height % 2, 0);
    }
}

fn render_frame(window: &MinimalSoftwareWindow, buffer: &mut [Rgb8Pixel]) {
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_rendering_rotation(RenderingRotation::Rotate90);
        renderer.set_dirty_region_alignment(DirtyRegionAlignment::new(4, 2));
        let region = renderer.render(buffer, HEIGHT);
        assert_aligned(&region);
    }));
}

// Rotate90 maps the window pixel (x, y) to the buffer pixel (HEIGHT - 1 - y, x).
fn buffer_index(x: usize, y: usize) -> usize {
    x * HEIGHT + (HEIGHT - 1 - y)
}

fn assert_frame(buffer: &[Rgb8Pixel], white_x: usize) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let expected = if x == white_x && y == 3 {
                Rgb8Pixel::new(255, 255, 255)
            } else if x == 2 && y == 3 {
                Rgb8Pixel::new(0, 255, 0)
            } else {
                Rgb8Pixel::new(0, 0, 0)
            };
            assert_eq!(buffer[buffer_index(x, y)], expected, "unexpected pixel at ({x}, {y})");
        }
    }
}

#[test]
fn alignment_uses_panel_axes_when_rotated() {
    slint::slint! {
        export component App inherits Window {
            in property <int> rect-x: 3;
            background: black;
            Rectangle {
                x: 2phx;
                y: 3phx;
                width: 1phx;
                height: 1phx;
                background: #00ff00;
            }
            Rectangle {
                x: root.rect-x * 1phx;
                y: 3phx;
                width: 1phx;
                height: 1phx;
                background: white;
            }
        }
    }

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let ui = App::new().unwrap();
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    ui.show().unwrap();

    let mut buffer = vec![Rgb8Pixel::new(0x55, 0x55, 0x55); WIDTH * HEIGHT];

    render_frame(&window, &mut buffer);
    assert_frame(&buffer, 3);

    for x in [5, 7, 9, 11] {
        ui.set_rect_x(x as i32);
        render_frame(&window, &mut buffer);
        assert_frame(&buffer, x);
    }
}
