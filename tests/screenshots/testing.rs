// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::rc::Rc;

use i_slint_core::{
    graphics::{
        euclid::{Box2D, Point2D},
        Rgb8Pixel, SharedPixelBuffer,
    },
    renderer::Renderer,
    software_renderer::{LineBufferProvider, MinimalSoftwareWindow},
};

pub struct SwrTestingBackend {
    window: Rc<MinimalSoftwareWindow<0>>,
}

impl i_slint_core::platform::Platform for SwrTestingBackend {
    fn create_window_adapter(&self) -> Rc<dyn i_slint_core::platform::WindowAdapter> {
        self.window.clone()
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

pub fn init_swr() -> std::rc::Rc<MinimalSoftwareWindow<0>> {
    let window = MinimalSoftwareWindow::new();

    i_slint_core::platform::set_platform(Box::new(SwrTestingBackend { window: window.clone() }))
        .unwrap();

    window
}

pub fn image_buffer(path: &str) -> SharedPixelBuffer<Rgb8Pixel> {
    let image = image::open(path).expect("Cannot open image.").into_rgb8();

    SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(image.as_raw(), image.width(), image.height())
}

pub fn screenshot(window: std::rc::Rc<MinimalSoftwareWindow<0>>) -> SharedPixelBuffer<Rgb8Pixel> {
    let size = window.size();
    let width = size.width;
    let height = size.height;

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);

    // render to buffer
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.mark_dirty_region(Box2D::new(
            Point2D::new(0., 0.),
            Point2D::new(width as f32, height as f32),
        ));
        renderer.render(buffer.make_mut_slice(), width as usize);
    });

    buffer
}

struct TestingLineBuffer<'a> {
    buffer: &'a mut [Rgb8Pixel],
}

impl<'a> LineBufferProvider for TestingLineBuffer<'a> {
    type TargetPixel = Rgb8Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        let start = line * range.len();
        let end = start + range.len();
        render_fn(&mut self.buffer[(start..end)]);
    }
}

fn color_difference(lhs: &Rgb8Pixel, rhs: &Rgb8Pixel) -> f32 {
    ((rhs.r as f32 - lhs.r as f32).powf(2.)
        + (rhs.g as f32 - lhs.g as f32).powf(2.)
        + (rhs.b as f32 - lhs.b as f32).powf(2.))
    .sqrt()
}

fn compare_images(
    reference: SharedPixelBuffer<Rgb8Pixel>,
    screenshot: SharedPixelBuffer<Rgb8Pixel>,
) {
    assert_eq!(reference.size(), screenshot.size());
    if reference.as_bytes() != screenshot.as_bytes() {
        let (failed_pixel_count, max_color_difference) =
            reference.as_slice().iter().zip(screenshot.as_slice().iter()).fold(
                (0, 0.0f32),
                |(failure_count, max_color_difference), (reference_pixel, screenshot_pixel)| {
                    (
                        failure_count + (reference_pixel != screenshot_pixel) as usize,
                        max_color_difference
                            .max(color_difference(reference_pixel, screenshot_pixel)),
                    )
                },
            );
        eprintln!(
            "Percentage of pixels that are different: {}",
            failed_pixel_count * 100 / reference.as_slice().len()
        );
        eprintln!("Maximum color difference: {}", max_color_difference);
    }
    assert_eq!(reference.as_bytes(), screenshot.as_bytes());
}

pub fn assert_with_render(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<0>>) {
    compare_images(image_buffer(path), screenshot(window));
}

pub fn assert_with_render_by_line(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<0>>) {
    compare_images(image_buffer(path), screenshot_render_by_line(window));
}

pub fn screenshot_render_by_line(
    window: std::rc::Rc<MinimalSoftwareWindow<0>>,
) -> SharedPixelBuffer<Rgb8Pixel> {
    let size = window.size();
    let width = size.width;
    let height = size.height;

    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width as u32, height as u32);

    // render to buffer
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.mark_dirty_region(Box2D::new(
            Point2D::new(0., 0.),
            Point2D::new(width as f32, height as f32),
        ));
        renderer.render_by_line(TestingLineBuffer { buffer: buffer.make_mut_slice() });
    });

    buffer
}

pub fn save_screenshot(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<0>>) {
    let buffer = screenshot(window.clone());
    image::save_buffer(
        path,
        buffer.as_bytes(),
        window.size().width,
        window.size().height,
        image::ColorType::Rgb8,
    )
    .unwrap();
}
