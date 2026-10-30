// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Draws an image of each of the optional pixel formats with the software renderer, which is
//! what those formats are for: handing the buffer to Slint without expanding it first.

use std::rc::Rc;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{Rgb8Pixel, SharedPixelBuffer};

const SIZE: usize = 4;

struct TestPlatform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

slint::slint! {
    #[edition = "1.19"]
    export component TestCase inherits Window {
        in property <image> source;
        in property <brush> tint;
        background: black;
        Image {
            x: 0; y: 0;
            width: 4phx;
            height: 4phx;
            source: root.source;
            colorize: root.tint;
            image-rendering: pixelated;
        }
    }
}

fn render(window: &MinimalSoftwareWindow) -> Vec<Rgb8Pixel> {
    let mut buffer = vec![Rgb8Pixel::new(0, 0, 0); SIZE * SIZE];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, SIZE);
    }));
    buffer
}

#[cfg(feature = "owned-pre-rendered-resources")]
struct LineBuffer<'a>(&'a mut [Rgb8Pixel]);

#[cfg(feature = "owned-pre-rendered-resources")]
impl slint::platform::software_renderer::LineBufferProvider for LineBuffer<'_> {
    type TargetPixel = Rgb8Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        render_fn(&mut self.0[line * SIZE..][range]);
    }
}

#[cfg(feature = "owned-pre-rendered-resources")]
fn render_by_line(window: &MinimalSoftwareWindow) -> Vec<Rgb8Pixel> {
    let mut buffer = vec![Rgb8Pixel::new(0, 0, 0); SIZE * SIZE];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.render_by_line(LineBuffer(&mut buffer));
    }));
    buffer
}

#[test]
fn optional_pixel_formats_render() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let ui = TestCase::new().unwrap();
    window.set_size(slint::PhysicalSize::new(SIZE as u32, SIZE as u32));
    ui.show().unwrap();

    // Gray8: the luminance is written to all three channels.
    let mut gray = SharedPixelBuffer::<slint::Gray8Pixel>::new(SIZE as u32, SIZE as u32);
    for (i, p) in gray.make_mut_slice().iter_mut().enumerate() {
        *p = slint::Gray8Pixel(if i % 2 == 0 { 0x00 } else { 0xff });
    }
    ui.set_source(slint::Image::from_gray8(gray));
    let rendered = render(&window);
    for (i, p) in rendered.iter().enumerate() {
        let v = if i % 2 == 0 { 0x00 } else { 0xff };
        assert_eq!(*p, Rgb8Pixel::new(v, v, v), "gray8 mismatch at {i}");
    }

    // RGB565: a saturated component reaches 0xff on an RGB8 target.
    let mut rgb565 = SharedPixelBuffer::<slint::Rgb565Pixel>::new(SIZE as u32, SIZE as u32);
    for (i, p) in rgb565.make_mut_slice().iter_mut().enumerate() {
        *p = match i % 3 {
            0 => slint::Rgb565Pixel::from_rgb(0xff, 0, 0),
            1 => slint::Rgb565Pixel::from_rgb(0, 0xff, 0),
            _ => slint::Rgb565Pixel::from_rgb(0, 0, 0xff),
        };
    }
    ui.set_source(slint::Image::from_rgb565(rgb565));
    let rendered = render(&window);

    for (i, p) in rendered.iter().enumerate() {
        let expected = match i % 3 {
            0 => Rgb8Pixel::new(0xff, 0, 0),
            1 => Rgb8Pixel::new(0, 0xff, 0),
            _ => Rgb8Pixel::new(0, 0, 0xff),
        };
        assert_eq!(*p, expected, "rgb565 mismatch at {i}");
    }

    #[cfg(feature = "owned-pre-rendered-resources")]
    alpha_map_renders_in_its_color_or_the_colorize_color(&window, &ui);
}

#[cfg(feature = "owned-pre-rendered-resources")]
fn alpha_map_renders_in_its_color_or_the_colorize_color(
    window: &MinimalSoftwareWindow,
    ui: &TestCase,
) {
    use i_slint_core::graphics::{IntRect, IntSize, StaticTexture, StaticTextures};
    let size = IntSize::new(SIZE as u32, SIZE as u32);
    let textures = StaticTextures {
        size,
        original_size: size,
        data: (0..SIZE * SIZE).map(|i| if i % 2 == 0 { 0x00 } else { 0xff }).collect(),
        textures: vec![StaticTexture {
            rect: IntRect::from_size(size.cast()),
            format: i_slint_core::graphics::TexturePixelFormat::AlphaMap,
            color: 0xff00ff00,
            index: 0,
        }],
    };
    let image =
        slint::Image::from(i_slint_core::graphics::ImageInner::OwnedTextures(Rc::new(textures)));
    let rgba = image.to_rgba8().unwrap();
    let premultiplied = image.to_rgba8_premultiplied().unwrap();
    for (i, (p, pm)) in rgba.as_slice().iter().zip(premultiplied.as_slice()).enumerate() {
        let a = if i % 2 == 0 { 0x00 } else { 0xff };
        assert_eq!(*p, slint::Rgba8Pixel::new(0, a, 0, a), "rgba of the alpha map at {i}");
        assert_eq!(*pm, *p, "premultiplied alpha map at {i}");
    }
    ui.set_source(image);
    for (tint, color) in [
        (slint::Color::default(), Rgb8Pixel::new(0, 0xff, 0)),
        (slint::Color::from_rgb_u8(0xff, 0, 0), Rgb8Pixel::new(0xff, 0, 0)),
    ] {
        ui.set_tint(tint.into());
        for rendered in [render(window), render_by_line(window)] {
            for (i, p) in rendered.iter().enumerate() {
                let expected = if i % 2 == 0 { Rgb8Pixel::new(0, 0, 0) } else { color };
                assert_eq!(*p, expected, "alpha map mismatch at {i} with {tint:?}");
            }
        }
    }
}
