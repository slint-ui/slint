// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore powf

use crate::testing::{TestCaseOptions, compare_images};
use i_slint_core::graphics::{IntRect, Rgb8Pixel, SharedPixelBuffer, euclid};
use i_slint_core::lengths::LogicalRect;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::RendererSealed;
use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RenderingRotation,
};
use std::rc::Rc;

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
    // The build script registers SLINT_DEFAULT_FONT and SLINT_FONT_PATH via
    // `cargo:rustc-env=`, which only exposes them to `env!()` at compile time —
    // the test binary's runtime environment is empty. Propagate them so
    // `i_slint_common::sharedfontique` can register NotoSans as the primary
    // vector font, enabling the parley rendering path in the software renderer.
    // Safety: cargo runs each test binary in its own process; no other threads
    // are touching the environment at this point.
    unsafe {
        std::env::set_var("SLINT_DEFAULT_FONT", env!("SLINT_DEFAULT_FONT"));
        std::env::set_var("SLINT_FONT_PATH", env!("SLINT_FONT_PATH"));
    }

    let window = MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
    );

    i_slint_core::platform::set_platform(Box::new(SwrTestingBackend { window: window.clone() }))
        .unwrap();

    window
}

pub fn screenshot(
    window: Rc<MinimalSoftwareWindow>,
    rotated: RenderingRotation,
) -> SharedPixelBuffer<Rgb8Pixel> {
    let size = window.size();
    let sf = window.scale_factor();
    let width = size.width;
    let height = size.height;

    let mut buffer = match rotated {
        RenderingRotation::Rotate90 | RenderingRotation::Rotate270 => {
            SharedPixelBuffer::<Rgb8Pixel>::new(height, width)
        }
        _ => SharedPixelBuffer::<Rgb8Pixel>::new(width, height),
    };

    // render to buffer
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.mark_dirty_region(
            LogicalRect::from_size(euclid::size2(width as f32 / sf, height as f32 / sf)).into(),
        );
        renderer.set_rendering_rotation(rotated);
        let stride = buffer.width() as usize;
        renderer.render(buffer.make_mut_slice(), stride);
        renderer.set_rendering_rotation(RenderingRotation::NoRotation);
    });

    buffer
}

struct TestingLineBuffer<'a> {
    buffer: &'a mut [Rgb8Pixel],
    stride: usize,
    region: Option<IntRect>,
}

impl LineBufferProvider for TestingLineBuffer<'_> {
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
        render_fn(&mut self.buffer[start..end]);
    }
}

pub fn assert_with_render(
    path: &str,
    window: Rc<MinimalSoftwareWindow>,
    options: &TestCaseOptions,
) {
    for rotation in [
        RenderingRotation::NoRotation,
        RenderingRotation::Rotate180,
        RenderingRotation::Rotate90,
        RenderingRotation::Rotate270,
    ] {
        let rendering = screenshot(window.clone(), rotation);
        let argb8 = i_slint_core::graphics::Image::from_rgb8(rendering).to_rgba8().unwrap();
        if let Err(reason) = compare_images(path, &argb8, rotation, options) {
            panic!("Image comparison failure for {path} ({rotation:?}): {reason}");
        }
    }
}

pub fn assert_with_render_by_line(
    path: &str,
    window: Rc<MinimalSoftwareWindow>,
    options: &TestCaseOptions,
) {
    let s = window.size();
    let mut rendering = SharedPixelBuffer::<Rgb8Pixel>::new(s.width, s.height);

    screenshot_render_by_line(window.clone(), None, &mut rendering);

    let argb8 = i_slint_core::graphics::Image::from_rgb8(rendering.clone()).to_rgba8().unwrap();
    if let Err(reason) = compare_images(path, &argb8, RenderingRotation::NoRotation, options) {
        panic!("Image comparison failure for line-by-line rendering for {path}: {reason}");
    }

    // Try to render a clipped version (to simulate partial rendering) and it should be exactly the same
    let region = euclid::rect(s.width / 4, s.height / 4, s.width / 2, s.height / 2).cast::<usize>();
    for y in region.y_range() {
        let stride = rendering.width() as usize;
        // fill with garbage
        rendering.make_mut_slice()[y * stride..][region.x_range()].fill(Rgb8Pixel::new(
            ((y << 3) & 0xff) as u8,
            0,
            255,
        ));
    }
    screenshot_render_by_line(window, Some(region.cast()), &mut rendering);
    if !options.skip_clipping {
        let argb8 = i_slint_core::graphics::Image::from_rgb8(rendering).to_rgba8().unwrap();
        if let Err(reason) = compare_images(path, &argb8, RenderingRotation::NoRotation, options) {
            panic!(
                "Partial rendering image comparison failure for line-by-line rendering for {path}: {reason}"
            );
        }
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
            None => renderer.mark_dirty_region(
                LogicalRect::from_size(euclid::size2(
                    buffer.width() as f32 / window.scale_factor(),
                    buffer.height() as f32 / window.scale_factor(),
                ))
                .into(),
            ),
            Some(r) => renderer.mark_dirty_region(
                (euclid::Rect::from_untyped(&r.cast()) / window.scale_factor()).into(),
            ),
        }
        renderer.render_by_line(TestingLineBuffer {
            stride: buffer.width() as usize,
            buffer: buffer.make_mut_slice(),
            region,
        });
    });
}
