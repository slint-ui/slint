// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Path rendering support for the software renderer using zeno

use super::PhysicalRect;
use super::draw_functions::{LineClip, PremultipliedRgbaColor, TargetPixel, interpolate_color};
use super::{
    AnyGradientCommand, PhysicalLength, PhysicalRegion, RoundedClip, ShapeClip, shape_clips,
};
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

/// Common rendering logic for both filled and stroked paths.
///
/// Rasterizes the path's mask once and draws it into each rectangle of `region`.
/// `clips` is scratch space for the rounded clips of one rectangle.
fn render_path_with_style<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    region: &PhysicalRegion,
    rounded_clips: &[RoundedClip],
    clips: &mut Vec<ShapeClip>,
    paint: &Paint,
    style: zeno::Style,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    let path_width = path_geometry.size.width as usize;
    let path_height = path_geometry.size.height as usize;
    if path_width == 0 || path_height == 0 || region.iter_box().next().is_none() {
        return;
    }

    // The mask is always rendered at the full path size: rasterizing a translated or cropped
    // path rounds differently, so a partial repaint would differ from a full one.
    let mut mask_buffer = vec![0u8; path_width * path_height];
    Mask::new(commands)
        .size(path_width as u32, path_height as u32)
        .style(style)
        .render_into(&mut mask_buffer, None);

    for clip_geometry in region.iter_box().map(|r| r.to_rect()) {
        clips.clear();
        clips.extend(shape_clips(rounded_clips, &clip_geometry));
        draw_mask(
            &mask_buffer,
            (path_geometry.origin.x as isize, path_geometry.origin.y as isize, path_width),
            &clip_geometry,
            clips,
            paint,
            buffer,
        );
    }
}

/// Draws `mask` within `clip_geometry`.
/// `mask` is `(x, y, width)`: the screen position of its top left corner and its row length.
fn draw_mask<T: TargetPixel>(
    mask_buffer: &[u8],
    (mask_x_origin, mask_y_origin, mask_width): (isize, isize, usize),
    clip_geometry: &PhysicalRect,
    clips: &[ShapeClip],
    paint: &Paint,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    let mask_height = mask_buffer.len() / mask_width;
    let x_start = (clip_geometry.min_x() as isize).max(mask_x_origin).max(0);
    let x_end = (clip_geometry.max_x() as isize)
        .min(mask_x_origin + mask_width as isize)
        .min(buffer.line_slice(0).len() as isize);
    let y_start = (clip_geometry.min_y() as isize).max(mask_y_origin).max(0);
    let y_end = (clip_geometry.max_y() as isize)
        .min(mask_y_origin + mask_height as isize)
        .min(buffer.num_lines() as isize);
    if x_start >= x_end {
        return;
    }
    let mask_x = (x_start - mask_x_origin) as usize;
    let (x_start, x_end, y_start, y_end) =
        (x_start as usize, x_end as usize, y_start as usize, y_end as usize);
    // A row's mask scaled by the rounded clips, for the rows that cross one of their corners.
    let mut clipped_row = vec![0u8; if clips.is_empty() { 0 } else { x_end - x_start }];

    for screen_y in y_start..y_end {
        let row_start = (screen_y as isize - mask_y_origin) as usize * mask_width + mask_x;
        let mut mask_row = &mask_buffer[row_start..row_start + (x_end - x_start)];
        if let Some(line_clip) = LineClip::new(
            clip_geometry,
            PhysicalLength::new(screen_y as i16),
            clips,
            x_end - x_start,
            (x_start as isize - clip_geometry.min_x() as isize) as i16,
            (clip_geometry.max_x() as isize - x_end as isize) as i16,
        ) {
            clipped_row.fill(0);
            let inner = line_clip.inner.clone();
            clipped_row[inner.clone()].copy_from_slice(&mask_row[inner]);
            line_clip.for_each_edge_chunk(|chunk, coverage| {
                for (x, c) in chunk.zip(coverage) {
                    clipped_row[x] = (mask_row[x] as u16 * *c as u16 / 255) as u8;
                }
            });
            mask_row = &clipped_row;
        }
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
                    let opaque = mask_row[run_start] == u8::MAX;
                    run_end = mask_row[run_start..]
                        .iter()
                        .position(|&c| c == 0 || (c == u8::MAX) != opaque)
                        .map_or(mask_row.len(), |len| run_start + len);
                    let run = x_start + run_start..x_start + run_end;
                    if opaque {
                        for (rect, gradient) in gradients {
                            gradient.as_command::<T>().draw_line(
                                rect,
                                line_number,
                                &mut line[run.clone()],
                                run.start as i16 - rect.min_x(),
                                rect.max_x() - run.end as i16,
                            );
                        }
                        continue;
                    }
                    blend_with_coverage(
                        line,
                        run,
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

/// `coverage(x)` is 0 to 255.
/// `draw` draws the gradient for a sub-range of `range` into a cleared scratch buffer.
fn blend_with_coverage<T: TargetPixel>(
    buffer: &mut [T],
    range: core::ops::Range<usize>,
    mut draw: impl FnMut(&mut [PremultipliedRgbaColor], core::ops::Range<usize>),
    coverage: impl Fn(usize) -> u32,
) {
    // Bounds the stack scratch buffer.
    const CHUNK: usize = 16;
    let mut scratch = [PremultipliedRgbaColor::default(); CHUNK];
    for start in range.clone().step_by(CHUNK) {
        let chunk = start..(start + CHUNK).min(range.end);
        let scratch = &mut scratch[..chunk.len()];
        scratch.fill(PremultipliedRgbaColor::default());
        draw(scratch, chunk.clone());
        for (x, color) in chunk.zip(scratch.iter()) {
            let color = interpolate_color(coverage(x), PremultipliedRgbaColor::default(), *color);
            buffer[x].blend(color);
        }
    }
}

/// Render a filled path
///
/// * `commands` - The path commands to render
/// * `path_geometry` - The full bounding box of the path in screen coordinates
/// * `region` - Where the path should be rendered (intersection of path, clip and dirty region)
/// * `rounded_clips` - The rounded clips the path is drawn in
/// * `clips` - Scratch space for the rounded clips of one rectangle of `region`
/// * `paint` - What to fill the path's coverage with
/// * `fill_rule` - How overlapping and nested contours determine the inside of the path
/// * `buffer` - The target pixel buffer
pub fn render_filled_path<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    region: &PhysicalRegion,
    rounded_clips: &[RoundedClip],
    clips: &mut Vec<ShapeClip>,
    paint: &Paint,
    fill_rule: i_slint_core::items::FillRule,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    let fill = match fill_rule {
        i_slint_core::items::FillRule::Evenodd => Fill::EvenOdd,
        i_slint_core::items::FillRule::Nonzero | _ => Fill::NonZero,
    };
    render_path_with_style(
        commands,
        path_geometry,
        region,
        rounded_clips,
        clips,
        paint,
        zeno::Style::Fill(fill),
        buffer,
    );
}

#[derive(Clone, Copy)]
pub struct StrokeStyle<'a> {
    pub width: f32,
    pub line_cap: i_slint_core::items::LineCap,
    pub line_join: i_slint_core::items::LineJoin,
    pub miter_limit: f32,
    pub dash_array: &'a [f32],
    pub dash_offset: f32,
}

/// Render a stroked path
///
/// * `commands` - The path commands to render
/// * `path_geometry` - The full bounding box of the path in screen coordinates
/// * `region` - Where the path should be rendered (intersection of path, clip and dirty region)
/// * `rounded_clips` - The rounded clips the path is drawn in
/// * `clips` - Scratch space for the rounded clips of one rectangle of `region`
/// * `paint` - What to fill the path's coverage with
/// * `stroke_width` - The width of the stroke
/// * `buffer` - The target pixel buffer
pub fn render_stroked_path<T: TargetPixel>(
    commands: &[Command],
    path_geometry: &PhysicalRect,
    region: &PhysicalRegion,
    rounded_clips: &[RoundedClip],
    clips: &mut Vec<ShapeClip>,
    paint: &Paint,
    stroke_style: StrokeStyle<'_>,
    buffer: &mut impl crate::target_pixel_buffer::TargetPixelBuffer<TargetPixel = T>,
) {
    let mut stroke = Stroke::new(stroke_style.width);
    stroke
        .cap(match stroke_style.line_cap {
            i_slint_core::items::LineCap::Round => Cap::Round,
            i_slint_core::items::LineCap::Square => Cap::Square,
            i_slint_core::items::LineCap::Butt | _ => Cap::Butt,
        })
        .join(match stroke_style.line_join {
            i_slint_core::items::LineJoin::Round => Join::Round,
            i_slint_core::items::LineJoin::Bevel => Join::Bevel,
            i_slint_core::items::LineJoin::Miter | _ => Join::Miter,
        })
        .miter_limit(stroke_style.miter_limit)
        .dash(stroke_style.dash_array, stroke_style.dash_offset);

    let style = Style::Stroke(stroke);
    render_path_with_style(
        commands,
        path_geometry,
        region,
        rounded_clips,
        clips,
        paint,
        style,
        buffer,
    );
}
