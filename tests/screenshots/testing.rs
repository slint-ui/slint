// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore powf

use std::rc::Rc;

use crossterm::style::Stylize;

use i_slint_core::{
    graphics::{
        euclid::{self, Box2D, Point2D},
        IntRect, Rgb8Pixel, SharedPixelBuffer,
    },
    item_rendering::DirtyRegion,
    platform::PlatformError,
    renderer::Renderer,
    software_renderer::{LineBufferProvider, MinimalSoftwareWindow},
    window::WindowAdapterSealed,
};

pub struct SwrTestingBackend {
    window: Rc<MinimalSoftwareWindow>,
}

impl i_slint_core::platform::Platform for SwrTestingBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn i_slint_core::platform::WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

pub fn init_swr() -> Rc<MinimalSoftwareWindow> {
    let window = MinimalSoftwareWindow::new(
        i_slint_core::software_renderer::RepaintBufferType::ReusedBuffer,
    );

    i_slint_core::platform::set_platform(Box::new(SwrTestingBackend { window: window.clone() }))
        .unwrap();

    window
}

pub fn image_buffer(path: &str) -> Result<SharedPixelBuffer<Rgb8Pixel>, image::ImageError> {
    image::open(path).map(|image| {
        let image = image.into_rgb8();
        SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        )
    })
}

pub fn screenshot(window: Rc<MinimalSoftwareWindow>) -> SharedPixelBuffer<Rgb8Pixel> {
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
    stride: usize,
    region: Option<IntRect>,
}

impl<'a> LineBufferProvider for TestingLineBuffer<'a> {
    type TargetPixel = Rgb8Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        if let Some(r) = self.region.map(|r| r.cast::<usize>()) {
            assert!(r.y_range().contains(&line), "line {line} out of range {r:?}");
            assert_eq!(r.cast().x_range(), range);
        }
        let start = line * self.stride + range.start;
        let end = line * self.stride + range.end;
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
    reference_path: &str,
    screenshot: &SharedPixelBuffer<Rgb8Pixel>,
) -> Result<(), String> {
    let compare = || {
        let reference = image_buffer(reference_path)
            .map_err(|image_err| format!("error loading reference image: {image_err:#}"))?;

        if reference.size() != screenshot.size() {
            return Err(format!(
                "image sizes don't match. reference size {:#?} rendered size {:#?}",
                reference.size(),
                screenshot.size()
            ));
        }
        if reference.as_bytes() == screenshot.as_bytes() {
            return Ok(());
        }

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
        if max_color_difference < 1.75 {
            return Ok(());
        }

        for y in 0..screenshot.height() {
            for x in 0..screenshot.width() {
                let pa = reference.as_slice()[(y * reference.width() + x) as usize];
                let pb = screenshot.as_slice()[(y * screenshot.width() + x) as usize];
                let ca = crossterm::style::Color::Rgb { r: pa.r, g: pa.g, b: pa.b };
                let cb = crossterm::style::Color::Rgb { r: pb.r, g: pb.g, b: pb.b };
                if pa == pb {
                    eprint!("{}", crossterm::style::style("██").on(ca).with(cb));
                } else if color_difference(&pa, &pb) >= 1.75 {
                    eprint!(
                        "{}{}",
                        crossterm::style::style("•").on(ca).slow_blink().red(),
                        crossterm::style::style("•").on(cb).slow_blink().green()
                    );
                } else {
                    eprint!(
                        "{}{}",
                        crossterm::style::style(".").on(ca).slow_blink().red(),
                        crossterm::style::style(".").on(cb).slow_blink().green()
                    );
                }
            }
            eprintln!();
        }

        Err(format!("images are not equal. Percentage of pixels that are different: {}. Maximum color difference: {}", failed_pixel_count * 100 / reference.as_slice().len(), max_color_difference))
    };

    let result = compare();

    if result.is_err() && std::env::var("SLINT_CREATE_SCREENSHOTS").map_or(false, |var| var == "1")
    {
        eprintln!("saving rendered image as comparison to reference failed");
        image::save_buffer(
            reference_path,
            screenshot.as_bytes(),
            screenshot.width(),
            screenshot.height(),
            image::ColorType::Rgb8,
        )
        .unwrap();
    }

    result
}

pub fn assert_with_render(path: &str, window: Rc<MinimalSoftwareWindow>) {
    let rendering = screenshot(window);
    if let Err(reason) = compare_images(path, &rendering) {
        panic!("Image comparison failure for {path}: {reason}");
    }
}

pub fn assert_with_render_by_line(path: &str, window: Rc<MinimalSoftwareWindow>) {
    let s = window.size();
    let mut rendering = SharedPixelBuffer::<Rgb8Pixel>::new(s.width, s.height);

    screenshot_render_by_line(window.clone(), None, &mut rendering);
    if let Err(reason) = compare_images(path, &rendering) {
        panic!("Image comparison failure for line-by-line rendering for {path}: {reason}");
    }

    // Try to render a clipped version (to simulate partial rendering) and it should be exactly the same
    let region = euclid::rect(s.width / 4, s.height / 4, s.width / 2, s.height / 2).cast::<usize>();
    for y in region.y_range() {
        let stride = rendering.width() as usize;
        rendering.make_mut_slice()[y * stride..][region.x_range()].fill(Default::default());
    }
    screenshot_render_by_line(window, Some(region.cast()), &mut rendering);
    if let Err(reason) = compare_images(path, &rendering) {
        panic!("Partial rendeing image comparison failure for line-by-line rendering for {path}: {reason}");
    }
}

pub fn screenshot_render_by_line(
    window: Rc<MinimalSoftwareWindow>,
    region: Option<IntRect>,
    buffer: &mut SharedPixelBuffer<Rgb8Pixel>,
) {
    // render to buffer
    window.request_redraw();

    window.draw_if_needed(|renderer| {
        match region {
            None => renderer.mark_dirty_region(Box2D::new(
                euclid::point2(0., 0.),
                euclid::point2(buffer.width() as f32, buffer.height() as f32),
            )),
            Some(r) => renderer.mark_dirty_region(DirtyRegion::from_untyped(&r.to_box2d().cast())),
        }
        renderer.render_by_line(TestingLineBuffer {
            stride: buffer.width() as usize,
            buffer: buffer.make_mut_slice(),
            region,
        });
    });
}

pub fn save_screenshot(path: &str, window: Rc<MinimalSoftwareWindow>) {
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
