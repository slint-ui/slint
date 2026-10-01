// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Path rendering support for the software renderer using zeno

use super::PhysicalRect;
use super::draw_functions::{PremultipliedRgbaColor, TargetPixel, blend_with_coverage};
use super::{AnyGradientCommand, PhysicalLength};
use alloc::vec;
use alloc::vec::Vec;
use zeno::{Cap, Fill, Join, Mask, Stroke, Style};

pub use zeno::Command;

/// Convert Slint's PathDataIterator to zeno's Command format
pub fn convert_path_data_to_zeno(
    path_data: i_slint_core::graphics::PathDataIterator,
    rotation: crate::RotationInfo,
    scale_factor: i_slint_core::lengths::ScaleFactor,
    offset: euclid::Vector2D<f32, i_slint_core::lengths::PhysicalPx>,
) -> Vec<Command> {
    use crate::Transform as _;
    use i_slint_core::lengths::LogicalPx;
    use lyon_path::Event;
    let mut commands = Vec::new();

    let convert_point = |p| {
        let p = (euclid::Point2D::<f32, LogicalPx>::from_untyped(p) * scale_factor + offset)
            .transformed(rotation);
        zeno::Point::new(p.x, p.y)
    };

    for event in path_data.iter() {
        match event {
            Event::Begin { at } => {
                commands.push(Command::MoveTo(convert_point(at)));
            }
            Event::Line { to, .. } => {
                commands.push(Command::LineTo(convert_point(to)));
            }
            Event::Quadratic { ctrl, to, .. } => {
                commands.push(Command::QuadTo(convert_point(ctrl), convert_point(to)));
            }
            Event::Cubic { ctrl1, ctrl2, to, .. } => {
                commands.push(Command::CurveTo(
                    convert_point(ctrl1),
                    convert_point(ctrl2),
                    convert_point(to),
                ));
            }
            Event::End { close, .. } => {
                if close {
                    commands.push(Command::Close);
                }
            }
        }
    }

    commands
}

/// What a path's coverage mask is filled with.
pub enum Paint {
    Solid(PremultipliedRgbaColor),
    /// Gradient commands, each with the rect it's placed in.
    Gradient(Vec<(PhysicalRect, AnyGradientCommand)>),
}

/// Common rendering logic for both filled and stroked paths
fn render_path_with_style<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    clip_geometry: &PhysicalRect,
    paint: &Paint,
    style: zeno::Style,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    // The mask needs to be rendered at the full path size
    let path_width = path_geometry.size.width as usize;
    let path_height = path_geometry.size.height as usize;

    if path_width == 0 || path_height == 0 {
        return;
    }

    // Calculate the intersection region - only apply within clipped area
    // clip_geometry is relative to screen, path_geometry is also relative to screen
    let clip_x_start = clip_geometry.origin.x.max(0) as usize;
    let clip_y_start = clip_geometry.origin.y.max(0) as usize;
    let clip_x_end = (clip_geometry.max_x().max(0) as usize).min(buffer.line_slice(0).len());
    let clip_y_end = (clip_geometry.max_y().max(0) as usize).min(buffer.num_lines());

    let path_x_start = path_geometry.origin.x as isize;
    let path_y_start = path_geometry.origin.y as isize;

    let x_start = clip_x_start.max(path_x_start.max(0) as usize);
    let x_end = clip_x_end.min((path_x_start + path_width as isize).max(0) as usize);
    if x_start >= x_end {
        return;
    }
    let mask_x = (x_start as isize - path_x_start) as usize;

    let mut mask_buffer = vec![0u8; path_width * path_height];
    Mask::new(commands)
        .size(path_width as u32, path_height as u32)
        .style(style)
        .render_into(&mut mask_buffer, None);

    // Apply the mask only within the clipped region
    for screen_y in clip_y_start..clip_y_end {
        // Calculate the y coordinate in the mask buffer
        let mask_y = screen_y as isize - path_y_start;
        if mask_y < 0 || mask_y >= path_height as isize {
            continue;
        }

        let row_start = mask_y as usize * path_width + mask_x;
        let mask_row = &mask_buffer[row_start..row_start + (x_end - x_start)];
        let line = buffer.line_slice(screen_y);

        match paint {
            Paint::Solid(color) => {
                for (pixel, &coverage) in line[x_start..x_end].iter_mut().zip(mask_row) {
                    if coverage > 0 {
                        // Scale all color components by coverage to maintain premultiplication
                        let coverage = coverage as u16;
                        let alpha_color = PremultipliedRgbaColor {
                            red: ((color.red as u16 * coverage) / 255) as u8,
                            green: ((color.green as u16 * coverage) / 255) as u8,
                            blue: ((color.blue as u16 * coverage) / 255) as u8,
                            alpha: ((color.alpha as u16 * coverage) / 255) as u8,
                        };
                        T::blend(pixel, alpha_color);
                    }
                }
            }
            Paint::Gradient(gradients) => {
                let line_number = PhysicalLength::new(screen_y as i16);
                let mut run_end = 0;
                while let Some(skipped) = mask_row[run_end..].iter().position(|&c| c > 0) {
                    let run_start = run_end + skipped;
                    run_end = mask_row[run_start..]
                        .iter()
                        .position(|&c| c == 0)
                        .map_or(mask_row.len(), |len| run_start + len);
                    blend_with_coverage(
                        line,
                        x_start + run_start..x_start + run_end,
                        |scratch, chunk| {
                            for (rect, gradient) in gradients {
                                gradient.as_command::<T>().draw_scratch_line(
                                    rect,
                                    line_number,
                                    scratch,
                                    chunk.start as i16 - rect.min_x(),
                                    rect.max_x() - chunk.end as i16,
                                );
                            }
                        },
                        |x| mask_row[x - x_start] as u32,
                    );
                }
            }
        }
    }
}

/// Render a filled path
///
/// * `commands` - The path commands to render
/// * `path_geometry` - The full bounding box of the path in screen coordinates
/// * `clip_geometry` - The clipped region where the path should be rendered (intersection of path and clip)
/// * `paint` - What to fill the path's coverage with
/// * `buffer` - The target pixel buffer
pub fn render_filled_path<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    clip_geometry: &PhysicalRect,
    paint: &Paint,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    render_path_with_style(
        commands,
        path_geometry,
        clip_geometry,
        paint,
        zeno::Style::Fill(Fill::NonZero),
        buffer,
    );
}

/// Render a stroked path
///
/// * `commands` - The path commands to render
/// * `path_geometry` - The full bounding box of the path in screen coordinates
/// * `clip_geometry` - The clipped region where the path should be rendered (intersection of path and clip)
/// * `paint` - What to fill the path's coverage with
/// * `stroke_width` - The width of the stroke
/// * `buffer` - The target pixel buffer
pub fn render_stroked_path<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    clip_geometry: &PhysicalRect,
    paint: &Paint,
    stroke_width: f32,
    stroke_line_cap: i_slint_core::items::LineCap,
    stroke_line_join: i_slint_core::items::LineJoin,
    stroke_miter_limit: f32,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    let mut stroke = Stroke::new(stroke_width);
    stroke
        .cap(match stroke_line_cap {
            i_slint_core::items::LineCap::Round => Cap::Round,
            i_slint_core::items::LineCap::Square => Cap::Square,
            i_slint_core::items::LineCap::Butt | _ => Cap::Butt,
        })
        .join(match stroke_line_join {
            i_slint_core::items::LineJoin::Round => Join::Round,
            i_slint_core::items::LineJoin::Bevel => Join::Bevel,
            i_slint_core::items::LineJoin::Miter | _ => Join::Miter,
        })
        .miter_limit(stroke_miter_limit);
    let style = Style::Stroke(stroke);
    render_path_with_style(commands, path_geometry, clip_geometry, paint, style, buffer);
}
