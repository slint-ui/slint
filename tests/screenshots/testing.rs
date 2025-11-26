// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore powf

use crossterm::style::Stylize;
use i_slint_core::graphics::{Rgb8Pixel, SharedPixelBuffer};

#[cfg(feature = "software")]
pub use slint::platform::software_renderer::RenderingRotation;
#[cfg(not(feature = "software"))]
#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RenderingRotation {
    #[default]
    NoRotation,
    Rotate90,
    Rotate180,
    Rotate270,
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

fn color_difference(lhs: &Rgb8Pixel, rhs: &Rgb8Pixel) -> f32 {
    ((rhs.r as f32 - lhs.r as f32).powf(2.)
        + (rhs.g as f32 - lhs.g as f32).powf(2.)
        + (rhs.b as f32 - lhs.b as f32).powf(2.))
    .sqrt()
}

#[derive(Default, Clone)]
pub struct TestCaseOptions {
    /// How much we allow the maximum pixel difference to be for the base (non-rotated) case
    pub base_threshold: f32,

    /// How much we allow the maximum pixel difference to be when operating a screen rotation
    pub rotation_threshold: f32,

    /// When true, we don't compare screenshots rendered with clipping
    pub skip_clipping: bool,
}

pub fn compare_images(
    reference_path: &str,
    screenshot: &SharedPixelBuffer<Rgb8Pixel>,
    rotated: RenderingRotation,
    options: &TestCaseOptions,
) -> Result<(), String> {
    let compare = || {
        let reference = image_buffer(reference_path)
            .map_err(|image_err| format!("error loading reference image: {image_err:#}"))?;

        let mut ref_size = reference.size();
        if matches!(rotated, RenderingRotation::Rotate90 | RenderingRotation::Rotate270) {
            std::mem::swap(&mut ref_size.width, &mut ref_size.height);
        }
        if ref_size != screenshot.size() {
            return Err(format!(
                "image sizes don't match. reference size {:#?} rendered size {:#?}",
                reference.size(),
                screenshot.size()
            ));
        }
        if reference.as_bytes() == screenshot.as_bytes() && rotated != RenderingRotation::NoRotation
        {
            return Ok(());
        }

        let idx = |x: u32, y: u32| -> u32 {
            match rotated {
                RenderingRotation::Rotate90 => (reference.height() - x - 1) * reference.width() + y,
                RenderingRotation::Rotate180 => {
                    (reference.height() - y - 1) * reference.width() + reference.width() - x - 1
                }
                RenderingRotation::Rotate270 => x * reference.width() + reference.width() - y - 1,
                _ => y * reference.width() + x,
            }
        };

        let fold_pixel = |(failure_count, max_color_difference): (usize, f32),
                          (reference_pixel, screenshot_pixel)| {
            (
                failure_count + (reference_pixel != screenshot_pixel) as usize,
                max_color_difference.max(color_difference(reference_pixel, screenshot_pixel)),
            )
        };

        let (failed_pixel_count, max_color_difference) = if rotated != RenderingRotation::NoRotation
        {
            let mut failure_count = 0usize;
            let mut max_color_difference = 0.0f32;
            for y in 0..screenshot.height() {
                for x in 0..screenshot.width() {
                    let pa = &reference.as_slice()[idx(x, y) as usize];
                    let pb = &screenshot.as_slice()[(y * screenshot.width() + x) as usize];
                    (failure_count, max_color_difference) =
                        fold_pixel((failure_count, max_color_difference), (pa, pb));
                }
            }
            (failure_count, max_color_difference)
        } else {
            reference
                .as_slice()
                .iter()
                .zip(screenshot.as_slice().iter())
                .fold((0usize, 0.0f32), fold_pixel)
        };
        let percentage_different = failed_pixel_count * 100 / reference.as_slice().len();

        // For non-rotated images, use base_threshold if set, otherwise use default 0.1
        if rotated == RenderingRotation::NoRotation {
            let threshold = if options.base_threshold > 0.0 { options.base_threshold } else { 0.1 };
            if max_color_difference < threshold {
                return Ok(());
            }
        } else {
            // For rotated images, use rotation_threshold
            if percentage_different < 1 || max_color_difference < options.rotation_threshold {
                return Ok(());
            }
        }

        for y in 0..screenshot.height() {
            for x in 0..screenshot.width() {
                let pa = reference.as_slice()[idx(x, y) as usize];
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

    if result.is_err()
        && rotated == RenderingRotation::NoRotation
        && std::env::var("SLINT_CREATE_SCREENSHOTS").is_ok_and(|var| var == "1")
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
