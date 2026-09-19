// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::Rgb8Pixel;
use slint::platform::software_renderer::{
    DirtyRegionAlignment, LineBufferProvider, MinimalSoftwareWindow, PhysicalRegion,
    RepaintBufferType,
};
use slint::platform::{PlatformError, WindowAdapter};

const WIDTH: usize = 16;
const HEIGHT: usize = 8;

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

fn assert_aligned(region: &PhysicalRegion) {
    for (position, size) in region.iter() {
        assert_eq!(position.x % 2, 0);
        assert_eq!(position.y % 2, 0);
        assert_eq!(size.width % 2, 0);
        assert_eq!(size.height % 2, 0);
    }
}

fn render_frame(window: &MinimalSoftwareWindow, buffer: &mut [Rgb8Pixel]) {
    assert!(window.draw_if_needed(|renderer| {
        renderer.set_dirty_region_alignment(DirtyRegionAlignment::new(2, 2));
        let region = renderer.render(buffer, WIDTH);
        assert_aligned(&region);
    }));
}

struct FrameBuffer<'a>(&'a mut [Rgb8Pixel]);

impl LineBufferProvider for FrameBuffer<'_> {
    type TargetPixel = Rgb8Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        render_fn(&mut self.0[line * WIDTH..][range]);
    }
}

fn render_frame_by_line(
    window: &MinimalSoftwareWindow,
    buffer: &mut [Rgb8Pixel],
    switch_to_reused_buffer: bool,
) {
    assert!(window.draw_if_needed(|renderer| {
        if switch_to_reused_buffer {
            renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
        }
        let region = renderer.render_by_line(FrameBuffer(buffer));
        assert_aligned(&region);
    }));
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
            assert_eq!(buffer[y * WIDTH + x], expected, "unexpected pixel at ({x}, {y})");
        }
    }
}

#[test]
fn alignment_preserves_content_in_partial_rendering_modes() {
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

    let window = MinimalSoftwareWindow::new(RepaintBufferType::SwappedBuffers);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let ui = App::new().unwrap();
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    ui.show().unwrap();

    let mut buffers = [
        vec![Rgb8Pixel::new(0x55, 0x55, 0x55); WIDTH * HEIGHT],
        vec![Rgb8Pixel::new(0xaa, 0xaa, 0xaa); WIDTH * HEIGHT],
    ];

    render_frame(&window, &mut buffers[0]);
    assert_frame(&buffers[0], 3);

    window.request_redraw();
    render_frame(&window, &mut buffers[1]);
    assert_frame(&buffers[1], 3);

    for (frame, x) in [5, 7, 9, 11].into_iter().enumerate() {
        ui.set_rect_x(x as i32);
        let buffer = &mut buffers[frame % 2];
        render_frame(&window, buffer);
        assert_frame(buffer, x);
    }

    ui.set_rect_x(3);
    let mut line_buffer = vec![Rgb8Pixel::new(0x55, 0x55, 0x55); WIDTH * HEIGHT];
    render_frame_by_line(&window, &mut line_buffer, true);
    assert_frame(&line_buffer, 3);

    ui.set_rect_x(5);
    render_frame_by_line(&window, &mut line_buffer, false);
    assert_frame(&line_buffer, 5);
}
