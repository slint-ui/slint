// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Box shadows, blurred analytically line by line, without a texture.

use super::draw_functions::{PremultipliedRgbaColor, TargetPixel};
use super::{
    Fixed, PhysicalLength, PhysicalRect, PhysicalSize, ProcessScene, RotationInfo, Transform,
    target_pixel_buffer,
};
use alloc::vec::Vec;
use i_slint_core::graphics::BorderRadius;
use i_slint_core::graphics::boxshadow::BoxShadowOptions;
use i_slint_core::lengths::{PhysicalPx, PointLengths};
use i_slint_core::{Brush, Color};
#[allow(unused_imports)]
use num_traits::Float;

#[cfg(test)]
use super::{PhysicalPoint, RenderingRotation, assert_partial_render_is_identical, render_region};
#[cfg(test)]
use i_slint_core::graphics::boxshadow::drop_shadow_bounding_rect;
#[cfg(test)]
use i_slint_core::lengths::{LogicalBorderRadius, LogicalLength, LogicalRect, ScaleFactor};

/// Draws the drop shadow described by `options`,
/// with `origin` the physical position of the element plus the shadow's offset,
/// before the panel rotation.
///
/// `color` is the shadow color with the opacity applied, and `clip` is in rotated coordinates.
pub(super) fn process_drop_shadow(
    processor: &mut dyn ProcessScene,
    options: &BoxShadowOptions,
    origin: euclid::Point2D<f32, PhysicalPx>,
    color: Color,
    clip: &PhysicalRect,
    rotation: RotationInfo,
) {
    let Some((background, layout)) = &options.source else { return };
    if color.alpha() == 0 || options.shape_size().is_empty() {
        return;
    }
    let spread = options.spread.get();
    type Radius = BorderRadius<f32, PhysicalPx>;
    let mut shapes = [(euclid::Rect::zero(), Radius::default(), 0.); MAX_SHADOW_SHAPES];
    let mut shape_count = 0;
    let mut push = |rect: euclid::Rect<f32, PhysicalPx>, radius: Radius, weight: f32| {
        if weight != 0. && !rect.is_empty() {
            shapes[shape_count] = (rect, radius, weight);
            shape_count += 1;
        }
    };
    if let Some(radius) = options.opaque_source_radius() {
        push(
            euclid::Rect::new(euclid::point2(-spread, -spread), options.shape_size()),
            (radius + Radius::new_uniform(spread)).max(Default::default()),
            1.,
        );
    } else {
        // The blur is evaluated analytically, which needs each shape's alpha to be uniform,
        // so a translucent gradient counts as opaque.
        let alpha = |brush: &Brush| match brush {
            Brush::SolidColor(c) => c.alpha() as f32 / 255.,
            Brush::LinearGradient(_) | Brush::RadialGradient(_) | Brush::ConicGradient(_)
                if brush.is_transparent() =>
            {
                0.
            }
            _ => 1.,
        };
        let fill = alpha(background);
        let border = if layout.border_width.get() > 0. { alpha(&layout.border_color) } else { 0. };
        // The border is stroked centered on `border_rect`, between an outer edge O and an
        // inner edge I, over the fill F, with I ⊆ F ⊆ O. Its alpha is
        // fill·F + border·(O − I) − fill·border·(F − I).
        let half = layout.border_width.get() / 2.;
        let r = layout.border_radius;
        let grow = |r: f32| if r > 0. { r + half } else { 0. };
        push(layout.background_rect, layout.background_radius, fill * (1. - border));
        push(
            layout.border_rect.inflate(half, half),
            Radius::new(
                grow(r.top_left),
                grow(r.top_right),
                grow(r.bottom_right),
                grow(r.bottom_left),
            ),
            border,
        );
        push(
            layout.border_rect.inflate(-half, -half),
            r.inner(euclid::Length::new(half)),
            -border * (1. - fill),
        );
    }
    let shapes = &mut shapes[..shape_count];
    for (rect, radius, _) in shapes.iter_mut() {
        *radius = super::scale_overlapping_radii(*radius, rect.size).transformed(rotation);
        *rect = rect.translate(origin.to_vector()).transformed(rotation);
    }
    let blur = options.blur.get();

    if blur <= 0.
        && let [(shape, radius, weight)] = *shapes
    {
        let args = target_pixel_buffer::DrawRectangleArgs {
            x: shape.origin.x,
            y: shape.origin.y,
            width: shape.size.width,
            height: shape.size.height,
            top_left_radius: radius.top_left,
            top_right_radius: radius.top_right,
            bottom_right_radius: radius.bottom_right,
            bottom_left_radius: radius.bottom_left,
            border_width: 0.,
            background: Brush::SolidColor(color),
            border: Brush::default(),
            alpha: (weight * 255.).round() as u8,
            rotation: rotation.orientation,
        };
        processor.process_rectangle(&args, *clip);
        return;
    }

    // Without a blur, the Gaussian below still anti-aliases the edges by about a pixel.
    let pad = if blur > 0. { blur } else { 1. };
    let Some(support) =
        shapes.iter().map(|(rect, ..)| rect.inflate(pad, pad)).reduce(|a, b| a.union(&b))
    else {
        return;
    };
    let Some(geometry) =
        support.round_out().intersection(&clip.cast()).and_then(|r| r.try_cast::<i16>())
    else {
        return;
    };
    // The shadow is point sampled at pixel centers, which aliases once σ nears a pixel.
    // Adding the variance of a pixel's box filter, 1/12 px², keeps the edges anti-aliased.
    let sigma = (options.blur_sigma().powi(2) + 1. / 12.).sqrt();
    for (rect, ..) in shapes.iter_mut() {
        *rect = rect.translate(-geometry.origin.cast::<f32>().to_vector());
    }
    processor.process_box_shadow(
        geometry,
        BoxShadowCommand::new(geometry.size, shapes, sigma, color.into()),
    );
}

const GAUSSIAN_TAIL_STEPS: usize = 64;
const GAUSSIAN_CUTOFF: i32 = 3;
/// `Φ(+∞)` in the Q15 representation of [`Gaussian::cdf`].
const GAUSSIAN_ONE: u32 = 1 << 15;

/// The upper tail of the standard normal distribution, `Q(t) = 1 − Φ(t)`,
/// for `t = i / GAUSSIAN_TAIL_STEPS` in `[0, GAUSSIAN_CUTOFF]`, in Q15.
static GAUSSIAN_TAIL: [u16; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1] =
    gaussian_tail_table();

const fn gaussian_tail_table() -> [u16; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1] {
    let mut table = [0; GAUSSIAN_CUTOFF as usize * GAUSSIAN_TAIL_STEPS + 1];
    let mut i = 0;
    while i < table.len() {
        let q = gaussian_q(i as f64 / GAUSSIAN_TAIL_STEPS as f64);
        table[i] = (q * GAUSSIAN_ONE as f64 + 0.5) as u16;
        i += 1;
    }
    table
}

/// `Q(t) = 1 − Φ(t)` for `0 ≤ t ≤ 4`, in a const context.
const fn gaussian_q(t: f64) -> f64 {
    // cSpell: ignore Abramowitz Stegun erfc
    // Abramowitz and Stegun 7.1.26: erfc(x) ≈ poly(1 / (1 + p·x)) · e^(−x²),
    // with an absolute error below 1.5e-7. Q(t) = erfc(t / √2) / 2.
    let x = t / core::f64::consts::SQRT_2;
    let k = 1. / (1. + 0.3275911 * x);
    let poly = k
        * (0.254829592
            + k * (-0.284496736 + k * (1.421413741 + k * (-1.453152027 + k * 1.061405429))));
    0.5 * poly * exp_neg(x * x)
}

/// `e^(−y)` for `0 ≤ y ≤ 8`, in a const context.
const fn exp_neg(y: f64) -> f64 {
    // The Taylor series converges fast for e^(−y/16); squaring four times restores e^(−y).
    let z = -y / 16.;
    let mut sum = 1.;
    let mut term = 1.;
    let mut k = 1;
    while k < 16 {
        term *= z / k as f64;
        sum += term;
        k += 1;
    }
    let mut i = 0;
    while i < 4 {
        sum *= sum;
        i += 1;
    }
    sum
}

#[derive(Clone, Copy)]
struct Gaussian {
    sigma: Fixed<i32, 8>,
    /// `2^29 / sigma`, so `|d| · inv_sigma` stays below `2^31` wherever the CDF doesn't saturate.
    inv_sigma: u32,
}

impl Gaussian {
    fn new(sigma: Fixed<i32, 8>) -> Self {
        Self { sigma, inv_sigma: (1 << 29) / sigma.0 as u32 }
    }

    /// The distance beyond which [`Self::cdf`] saturates.
    fn cutoff(self) -> Fixed<i32, 8> {
        self.sigma * GAUSSIAN_CUTOFF
    }

    /// The CDF `Φ(d / σ)` in Q15.
    #[inline]
    fn cdf(self, d: Fixed<i32, 8>) -> u32 {
        let cutoff = self.cutoff();
        if d >= cutoff {
            return GAUSSIAN_ONE;
        }
        if d <= -cutoff {
            return 0;
        }
        // |d| / σ in Q29: the table index is its top bits, the interpolation fraction the next 10.
        let t = d.0.unsigned_abs() * self.inv_sigma;
        let index = (t >> 23) as usize;
        let fract = (t >> 13) & 0x3ff;
        let q0 = GAUSSIAN_TAIL[index] as u32;
        let q1 = GAUSSIAN_TAIL[index + 1] as u32;
        let q = q0 - (((q0 - q1) * fract) >> 10);
        if d >= Fixed(0) { GAUSSIAN_ONE - q } else { q }
    }
}

const HALF_PIXEL: Fixed<i32, 8> = Fixed(128);

/// The index of the first pixel whose center is at or after `v`.
#[inline]
fn first_pixel_at(v: Fixed<i32, 8>) -> i32 {
    ((v - HALF_PIXEL).0 + 255).div_euclid(256)
}

/// The horizontal coverage of the rows whose blur only reaches the shape's straight sides.
#[derive(Debug)]
struct StraightRowProfile {
    /// The coverage of the columns outside `inner`, in 1/255:
    /// first the columns before `inner`, then those after it.
    ramps: Vec<u8>,
    /// The columns whose coverage saturates.
    inner: core::ops::Range<i32>,
}

impl StraightRowProfile {
    fn new(width: i32, left: Fixed<i32, 8>, right: Fixed<i32, 8>, gaussian: Gaussian) -> Self {
        let cutoff = gaussian.cutoff();
        let start = first_pixel_at(left + cutoff).clamp(0, width);
        let end = first_pixel_at(right - cutoff + Fixed(1)).clamp(start, width);
        let ramps = (0..start)
            .chain(end..width)
            .map(|x| {
                let x_center = Fixed::from_integer(x) + HALF_PIXEL;
                let coverage =
                    gaussian.cdf(x_center - left).saturating_sub(gaussian.cdf(x_center - right));
                ((coverage * 255 + GAUSSIAN_ONE / 2) >> 15) as u8
            })
            .collect();
        Self { ramps, inner: start..end }
    }
}

/// A rounded rectangle whose blur adds `weight` times its coverage to a [`BoxShadowCommand`].
#[derive(Clone, Copy, Debug, Default)]
struct ShadowShape {
    left: Fixed<i32, 8>,
    top: Fixed<i32, 8>,
    right: Fixed<i32, 8>,
    bottom: Fixed<i32, 8>,
    radius: BorderRadius<Fixed<i32, 8>, PhysicalPx>,
    /// In Q15, negative to cut a shape out of the others.
    weight: i32,
}

impl ShadowShape {
    /// Where the top corner curves end and the bottom ones start.
    fn curve_bounds(&self) -> (Fixed<i32, 8>, Fixed<i32, 8>) {
        let radius = &self.radius;
        let top_curve_end = (self.top + radius.top_left.max(radius.top_right)).min(self.bottom);
        let bottom_curve_start =
            (self.bottom - radius.bottom_left.max(radius.bottom_right)).max(top_curve_end);
        (top_curve_end, bottom_curve_start)
    }

    /// Whether any of the `height` rows can be drawn from a [`StraightRowProfile`].
    fn has_straight_rows(&self, sigma: Fixed<i32, 8>, height: i32) -> bool {
        let cutoff = Gaussian::new(sigma).cutoff();
        let (top_curve_end, bottom_curve_start) = self.curve_bounds();
        let first = if top_curve_end > self.top {
            first_pixel_at(top_curve_end + cutoff).max(0)
        } else {
            0
        };
        let end = if bottom_curve_start < self.bottom {
            first_pixel_at(bottom_curve_start - cutoff + Fixed(1)).min(height)
        } else {
            height
        };
        first < end
    }
}

/// The most shapes a [`BoxShadowCommand`] combines.
const MAX_SHADOW_SHAPES: usize = 3;

/// A blurred drop shadow: a weighted sum of rounded rectangles, convolved with a Gaussian.
#[derive(Debug)]
pub struct BoxShadowCommand {
    shapes: [ShadowShape; MAX_SHADOW_SHAPES],
    shape_count: u8,
    sigma: Fixed<i32, 8>,
    color: PremultipliedRgbaColor,
    /// Only for a single shape.
    profile: Option<StraightRowProfile>,
}

impl BoxShadowCommand {
    // Together with an i16 geometry, these keep every edge within 2^29 / 256 px,
    // so the arithmetic of `draw_box_shadow_line` stays within i32.
    const MAX_SIGMA: Fixed<i32, 8> = Fixed(1 << 23);
    const MAX_RADIUS: Fixed<i32, 8> = Fixed(1 << 28);

    /// A shadow of standard deviation `sigma` for `shapes`, each a rectangle, its corner radii,
    /// and its weight in `[-1, 1]`.
    /// The shapes are relative to the origin of the geometry of `size` they're drawn in.
    pub(super) fn new(
        size: PhysicalSize,
        shapes: &[(euclid::Rect<f32, PhysicalPx>, BorderRadius<f32, PhysicalPx>, f32)],
        sigma: f32,
        mut color: PremultipliedRgbaColor,
    ) -> Self {
        assert!(shapes.len() <= MAX_SHADOW_SHAPES);
        let fixed = |v: f32| Fixed::<i32, 8>((v * 256.).round() as i32);
        let sigma = fixed(sigma).clamp(Fixed(1), Self::MAX_SIGMA);
        // An edge further out than the Gaussian's reach plus its corners' radii can't affect
        // any pixel of the geometry.
        let reach = Gaussian::new(sigma).cutoff();
        let width = Fixed::from_integer(size.width as i32);
        let height = Fixed::from_integer(size.height as i32);
        let mut command = Self {
            shapes: Default::default(),
            shape_count: shapes.len() as u8,
            sigma,
            color,
            profile: None,
        };
        for (shape, &(rect, radius, weight)) in command.shapes.iter_mut().zip(shapes) {
            let radius = BorderRadius::new(
                fixed(radius.top_left),
                fixed(radius.top_right),
                fixed(radius.bottom_right),
                fixed(radius.bottom_left),
            )
            .min(BorderRadius::new_uniform(Self::MAX_RADIUS));
            *shape = ShadowShape {
                left: fixed(rect.min_x()).max(-(reach + radius.top_left.max(radius.bottom_left))),
                top: fixed(rect.min_y()).max(-(reach + radius.top_left.max(radius.top_right))),
                right: fixed(rect.max_x())
                    .min(width + reach + radius.top_right.max(radius.bottom_right)),
                bottom: fixed(rect.max_y())
                    .min(height + reach + radius.bottom_left.max(radius.bottom_right)),
                radius,
                weight: (weight * GAUSSIAN_ONE as f32).round() as i32,
            };
        }
        if let [shape] = &mut command.shapes[..shapes.len()] {
            // A single shape's weight scales the color instead, which the profile needs.
            color = scale_color(color, (shape.weight.max(0) as u32) << 1).unwrap_or_default();
            command.color = color;
            shape.weight = GAUSSIAN_ONE as i32;
            command.profile = shape.has_straight_rows(sigma, size.height as i32).then(|| {
                StraightRowProfile::new(
                    size.width as i32,
                    shape.left,
                    shape.right,
                    Gaussian::new(sigma),
                )
            });
        }
        command
    }

    fn shapes(&self) -> &[ShadowShape] {
        &self.shapes[..self.shape_count as usize]
    }
}

/// `color` scaled by a Q16 `coverage`, or `None` when it rounds to transparent.
#[inline(always)]
fn scale_color(color: PremultipliedRgbaColor, coverage: u32) -> Option<PremultipliedRgbaColor> {
    // Q16, so that full coverage keeps the color and 255 · 2^16 fits in u32.
    let scale = |c: u8| ((c as u32 * coverage + (1 << 15)) >> 16) as u8;
    // Premultiplied components never exceed alpha, so a zero alpha is a zero color.
    let alpha = scale(color.alpha);
    (alpha > 0).then(|| PremultipliedRgbaColor {
        alpha,
        red: scale(color.red),
        green: scale(color.green),
        blue: scale(color.blue),
    })
}

/// Draws one line of a shadow row that only reaches the shape's straight sides,
/// with `weight` its vertical Gaussian weight in Q15.
// Kept out of line, to leave `draw_box_shadow_line` within LLVM's inlining budget.
#[inline(never)]
fn draw_straight_box_shadow_line(
    profile: &StraightRowProfile,
    weight: u32,
    color: PremultipliedRgbaColor,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    let first_x = extra_left_clip as i32;
    let len = line_buffer.len() as i32;
    let inner = &profile.inner;
    let index = |x: i32| (x - first_x).clamp(0, len) as usize;
    let (inner_start, inner_end) = (index(inner.start), index(inner.end));
    if let Some(c) = scale_color(color, 2 * weight) {
        TargetPixel::blend_slice(&mut line_buffer[inner_start..inner_end], c);
    }
    let (before, after) = profile.ramps.split_at(inner.start as usize);
    let before = &before[(first_x as usize).min(before.len())..];
    let after = &after[(first_x - inner.end).max(0) as usize..];
    let (head, tail) = line_buffer.split_at_mut(inner_end);
    let ramps = head[..inner_start].iter_mut().zip(before).chain(tail.iter_mut().zip(after));
    for (pixel, &coverage) in ramps {
        // 257 widens the 1/255 coverage to Q16; `weight` is Q15.
        if let Some(c) = scale_color(color, (weight * (coverage as u32 * 257) + (1 << 14)) >> 15) {
            pixel.blend(c);
        }
    }
}

const MAX_BANDS: usize = 16;

/// Neighboring slices of a shape on one line, approximated as a single rectangle.
#[derive(Clone, Copy, Default)]
struct Band {
    x0: Fixed<i32, 8>,
    x1: Fixed<i32, 8>,
    /// The vertical Gaussian weight in Q15, scaled by the shape's weight once complete.
    weight: i32,
    /// The first slice's edges, which later slices must stay close to.
    first: (Fixed<i32, 8>, Fixed<i32, 8>),
    /// The slices' edges, weighted.
    sum: (i64, i64),
    slices: u32,
}

/// How far a corner of radius `r` indents the shape's edge at `distance` from the corner's
/// horizontal side.
#[inline(always)]
fn corner_inset(r: Fixed<i32, 8>, distance: Fixed<i32, 8>) -> Fixed<i32, 8> {
    let dy = r - distance;
    if dy <= Fixed(0) {
        return Fixed(0);
    }
    // Cortex-M0+ has no 64-bit multiply, and radii below 256 px keep r² within u32.
    if r.0 < 1 << 16 {
        let (r, dy) = (r.0 as u32, dy.0 as u32);
        return Fixed((r - (r * r - dy * dy).isqrt()) as i32);
    }
    let (r, dy) = (r.0 as i64, dy.0 as i64);
    Fixed((r - ((r * r - dy * dy) as u64).isqrt() as i64) as i32)
}

/// Fills `bands` with the bands of `shape` on the line whose center is `y_center`,
/// and returns their count.
fn shape_bands(
    shape: &ShadowShape,
    gaussian: Gaussian,
    y_center: Fixed<i32, 8>,
    bands: &mut [Band],
) -> usize {
    const MAX_SLICES: i32 = 32;

    let ShadowShape { left, top, right, bottom, radius, weight: shape_weight } = *shape;
    let sigma = gaussian.sigma;
    let cutoff = gaussian.cutoff();
    let window_top = top.max(y_center - cutoff);
    let window_bottom = bottom.min(y_center + cutoff);
    if window_top >= window_bottom {
        return 0;
    }

    let (top_curve_end, bottom_curve_start) = shape.curve_bounds();
    let in_window = |a: Fixed<i32, 8>, b: Fixed<i32, 8>| (a.max(window_top), b.min(window_bottom));
    let intervals = [
        (in_window(top, top_curve_end), true),
        (in_window(top_curve_end, bottom_curve_start), false),
        (in_window(bottom_curve_start, bottom), true),
    ];
    let length = |(a, b): (Fixed<i32, 8>, Fixed<i32, 8>)| (b - a).max(Fixed(0));
    let curved_budget = MAX_SLICES - (length(intervals[1].0) > Fixed(0)) as i32;
    let curved_length = length(intervals[0].0) + length(intervals[2].0);
    // For why σ/4 slices merged within σ, see `box_shadow_bands_stay_close_to_the_exact_blur`.
    // Rounding up the slice count of each of the two curved intervals adds at most one slice
    // each, so reserve two.
    let slice_height =
        (sigma / 4).max(Fixed((curved_length.0 + curved_budget - 3) / (curved_budget - 2)));

    let max_bands = bands.len();
    let mut band_count: usize = 0;
    let mut edge = window_top;
    let mut edge_cdf = gaussian.cdf(y_center - edge);
    for ((a, b), curved) in intervals {
        if a >= b {
            continue;
        }
        debug_assert_eq!(a, edge);
        let n = if curved { (b - a + slice_height - Fixed(1)) / slice_height } else { 1 };
        for k in 1..=n {
            let next_edge = a + (b - a) * k / n;
            let next_cdf = gaussian.cdf(y_center - next_edge);
            let weight = (edge_cdf - next_cdf) as i32;
            let middle = (edge + next_edge) / 2;
            edge = next_edge;
            edge_cdf = next_cdf;
            if weight == 0 {
                continue;
            }
            let x0 = left
                + corner_inset(radius.top_left, middle - top)
                    .max(corner_inset(radius.bottom_left, bottom - middle));
            let x1 = right
                - corner_inset(radius.top_right, middle - top)
                    .max(corner_inset(radius.bottom_right, bottom - middle));
            let close = |a: Fixed<i32, 8>, b: Fixed<i32, 8>| (a - b).0.abs() <= sigma.0;
            match band_count.checked_sub(1).map(|last| &mut bands[last]) {
                Some(last)
                    if band_count == max_bands
                        || (close(last.first.0, x0) && close(last.first.1, x1)) =>
                {
                    last.weight += weight;
                    last.sum.0 += weight as i64 * x0.0 as i64;
                    last.sum.1 += weight as i64 * x1.0 as i64;
                    last.slices += 1;
                }
                _ => {
                    bands[band_count] = Band {
                        x0,
                        x1,
                        weight,
                        first: (x0, x1),
                        sum: (weight as i64 * x0.0 as i64, weight as i64 * x1.0 as i64),
                        slices: 1,
                    };
                    band_count += 1;
                }
            }
        }
    }
    for band in &mut bands[..band_count] {
        if band.slices > 1 {
            band.x0 = Fixed((band.sum.0 / band.weight as i64) as i32);
            band.x1 = Fixed((band.sum.1 / band.weight as i64) as i32);
        }
        // Both are at most 2^15, so the product fits.
        band.weight = (band.weight * shape_weight) >> 15;
    }
    band_count
}

/// Draws one line of a blurred drop shadow.
///
/// The rows of each shape within reach of the blur are cut into horizontal slices,
/// each approximated as a rectangle spanning the shape's width at the slice's middle.
/// Neighboring slices of nearly the same width merge into a band,
/// whose edges are the weighted means of its slices' edges.
/// A band's contribution is its vertical Gaussian weight times its horizontal coverage,
/// both exact.
/// Rows of a single shape out of reach of the corner curves are a single band, drawn from the
/// precomputed [`StraightRowProfile`].
pub(super) fn draw_box_shadow_line(
    span: &PhysicalRect,
    line: PhysicalLength,
    shadow: &BoxShadowCommand,
    line_buffer: &mut [impl TargetPixel],
    extra_left_clip: i16,
) {
    /// The cost of interpolating a pixel, in quarters of a band loop.
    const INTERPOLATION_COST: i32 = 3;

    let BoxShadowCommand { sigma, color, ref profile, .. } = *shadow;
    let gaussian = Gaussian::new(sigma);
    let cutoff = gaussian.cutoff();

    let y_center = Fixed::from_integer((line - span.origin.y_length()).get() as i32) + HALF_PIXEL;

    if let (Some(profile), [shape]) = (profile, shadow.shapes()) {
        let window_top = shape.top.max(y_center - cutoff);
        let window_bottom = shape.bottom.min(y_center + cutoff);
        let (top_curve_end, bottom_curve_start) = shape.curve_bounds();
        if window_top < window_bottom
            && window_top >= top_curve_end
            && window_bottom <= bottom_curve_start
        {
            let weight =
                gaussian.cdf(y_center - window_top) - gaussian.cdf(y_center - window_bottom);
            draw_straight_box_shadow_line(profile, weight, color, line_buffer, extra_left_clip);
            return;
        }
    }

    let mut bands = [Band::default(); MAX_SHADOW_SHAPES * MAX_BANDS];
    let mut band_count = 0;
    for shape in shadow.shapes() {
        band_count += shape_bands(shape, gaussian, y_center, &mut bands[band_count..][..MAX_BANDS]);
    }
    let bands = &bands[..band_count];
    // Pixels this far inside every band see the CDFs saturate, so their coverage is constant.
    let (Some(inner_begin), Some(inner_end)) =
        (bands.iter().map(|b| b.x0).max(), bands.iter().map(|b| b.x1).min())
    else {
        return;
    };
    let (inner_begin, inner_end) = (inner_begin + cutoff, inner_end - cutoff);

    // Coverage is in Q30: weights and horizontal coverages are both Q15, and the weights sum to
    // at most 1. Negative weights can round it below zero.
    let scaled_color =
        |coverage: i32| scale_color(color, (coverage.max(0) as u32 + (1 << 13)) >> 14);

    let first_x = extra_left_clip as i32;
    let len = line_buffer.len() as i32;
    let index = |v: Fixed<i32, 8>| (first_pixel_at(v) - first_x).clamp(0, len) as usize;
    let inner = if inner_begin <= inner_end {
        index(inner_begin)..index(inner_end + Fixed(1))
    } else {
        0..0
    };

    if let Some(c) = scaled_color(bands.iter().map(|b| b.weight).sum::<i32>() * GAUSSIAN_ONE as i32)
    {
        TargetPixel::blend_slice(&mut line_buffer[inner.clone()], c);
    }
    // Forced inline, and a loop rather than `Iterator::sum`:
    // LLVM outlines either on thumbv6m, which costs a call per pixel.
    #[inline(always)]
    fn band_coverage(bands: &[Band], gaussian: Gaussian, x: i32) -> i32 {
        let x_center = Fixed::from_integer(x) + HALF_PIXEL;
        let mut coverage = 0;
        for b in bands {
            coverage += b.weight
                * gaussian.cdf(x_center - b.x0).saturating_sub(gaussian.cdf(x_center - b.x1))
                    as i32;
        }
        coverage
    }
    // The coverage is smooth over σ, so evaluate it every σ/4 pixels and interpolate.
    // Interpolating skips the band loop for all but one pixel in `step`,
    // but costs more per pixel than a single band's loop.
    // The samples are anchored to the target buffer's origin, so they don't depend on the clip.
    let step = sigma.truncate() / 4;
    let step = if step >= 2 && 4 * band_count as i32 * (step - 1) >= INTERPOLATION_COST * step {
        step
    } else {
        1
    };
    let anchor = -(span.origin.x as i32);
    // `2^16 / step`, so the slope needs no division.
    let inv_step = (1 << 16) / step as i64;
    for range in [0..inner.start, inner.end..len as usize] {
        if step == 1 {
            for i in range {
                if let Some(c) = scaled_color(band_coverage(bands, gaussian, first_x + i as i32)) {
                    line_buffer[i].blend(c);
                }
            }
            continue;
        }
        // Interpolated in Q24, so the steps fit in i32.
        let q24 = |x: i32| band_coverage(bands, gaussian, x) >> 6;
        let mut i = range.start;
        let mut sample_x = anchor + (first_x + i as i32 - anchor).div_euclid(step) * step;
        let mut c1 = q24(sample_x);
        while i < range.end {
            let c0 = c1;
            c1 = q24(sample_x + step);
            let slope = (((c1 - c0) as i64 * inv_step) >> 16) as i32;
            let mut value = c0 + slope * (first_x + i as i32 - sample_x);
            let segment_end = ((sample_x + step - first_x) as usize).min(range.end);
            for pixel in &mut line_buffer[i..segment_end] {
                // The slope is rounded down, so a falling segment can undershoot zero.
                if let Some(c) = scaled_color(value.max(0) << 6) {
                    pixel.blend(c);
                }
                value += slope;
            }
            i = segment_end;
            sample_x += step;
        }
    }
}

#[test]
fn gaussian_cdf_is_symmetric_monotonic_and_saturating() {
    for sigma in [1, 3 * 256 + 17, 40 * 256] {
        let gaussian = Gaussian::new(Fixed(sigma));
        let cdf = |d: i32| gaussian.cdf(Fixed(d));
        let cutoff = gaussian.cutoff().0;
        assert_eq!(cdf(0), GAUSSIAN_ONE / 2);
        assert_eq!(cdf(cutoff), GAUSSIAN_ONE);
        assert_eq!(cdf(cutoff + 1000), GAUSSIAN_ONE);
        assert_eq!(cdf(-cutoff), 0);
        assert_eq!(cdf(-cutoff - 1000), 0);
        let step = (sigma / 97).max(1);
        let mut previous = 0;
        for d in (-cutoff - 5 * step..=cutoff + 5 * step).step_by(step as usize) {
            assert_eq!(cdf(d) + cdf(-d), GAUSSIAN_ONE, "sigma {sigma}, d {d}");
            assert!(cdf(d) >= previous, "sigma {sigma}, d {d}");
            previous = cdf(d);
        }
    }
}

#[test]
fn gaussian_tail_matches_reference_values() {
    // Q(t) · 2^15 for t = 0.5, 1, 2, 3
    for (t, expected) in [(0.5, 10110.2), (1., 5198.8), (2., 745.5), (3., 44.2)] {
        let entry = GAUSSIAN_TAIL[(t * GAUSSIAN_TAIL_STEPS as f64) as usize] as f64;
        assert!((entry - expected).abs() <= 1., "Q({t}) = {entry}, expected {expected}");
    }
    assert!(GAUSSIAN_TAIL.array_windows().all(|[a, b]| a >= b));
}

#[cfg(test)]
impl BoxShadowCommand {
    fn single(
        shape: ShadowShape,
        sigma: Fixed<i32, 8>,
        color: PremultipliedRgbaColor,
        profile: Option<StraightRowProfile>,
    ) -> Self {
        Self {
            shapes: [shape, Default::default(), Default::default()],
            shape_count: 1,
            sigma,
            color,
            profile,
        }
    }
}

#[test]
fn box_shadow_straight_row_is_separable() {
    use super::PhysicalPoint;
    let sigma = 4 * 256 + 37;
    let white = PremultipliedRgbaColor { red: 255, green: 255, blue: 255, alpha: 255 };
    let (left, right) = (Fixed(20 * 256 + 77), Fixed(60 * 256 + 3));
    let shape = ShadowShape {
        left,
        top: Fixed(20 * 256 + 5),
        right,
        bottom: Fixed(300 * 256),
        radius: BorderRadius::new_uniform(Fixed(10 * 256)),
        weight: GAUSSIAN_ONE as i32,
    };
    let shadow = BoxShadowCommand::single(
        shape,
        Fixed(sigma),
        white,
        Some(StraightRowProfile::new(80, left, right, Gaussian::new(Fixed(sigma)))),
    );
    let span = PhysicalRect::new(PhysicalPoint::new(0, 0), PhysicalSize::new(80, 320));
    // More than 3σ away from both corner curves.
    let line = 150;
    let cdf = |d: i32| Gaussian::new(Fixed(sigma)).cdf(Fixed(d));
    let y_center = line * 256 + 128;
    let coverage_y = cdf(y_center - shape.top.0) - cdf(y_center - shape.bottom.0);

    for extra_left_clip in [0, 13] {
        let mut buffer = [PremultipliedRgbaColor::default(); 80];
        let buffer = &mut buffer[extra_left_clip as usize..];
        draw_box_shadow_line(
            &span,
            PhysicalLength::new(line as i16),
            &shadow,
            buffer,
            extra_left_clip,
        );
        for (x, pixel) in (extra_left_clip as i32..).zip(buffer.iter()) {
            let x_center = x * 256 + 128;
            let coverage_x = cdf(x_center - shape.left.0) - cdf(x_center - shape.right.0);
            let expected =
                ((((coverage_x * coverage_y) + (1 << 13)) >> 14) * 255 + (1 << 15)) >> 16;
            // The profile's 8-bit coverage rounds once more.
            assert!(
                pixel.alpha.abs_diff(expected as u8) <= 1,
                "x {x}: {} vs {expected}",
                pixel.alpha
            );
        }
    }
}

#[test]
fn box_shadow_bands_stay_close_to_the_exact_blur() {
    use super::PhysicalPoint;
    let phi = |t: f64| {
        let q = gaussian_q(t.abs().min(4.));
        if t >= 0. { 1. - q } else { q }
    };
    let corner_inset = |r: f64, distance: f64| {
        if distance >= r { 0. } else { r - (r * r - (r - distance).powi(2)).sqrt() }
    };
    let white = PremultipliedRgbaColor { red: 255, green: 255, blue: 255, alpha: 255 };
    for (sigma, radius) in [
        (1., 40.),
        (0.6, 16.),
        (1., 16.),
        (2.3, 30.),
        (4.1, 25.),
        (7.7, 40.),
        (5.2, 6.),
        (3.3, 3.),
        (8.3, 30.),
        (12.4, 40.),
        (16.7, 20.),
        (24.2, 40.),
        (12.1, 8.),
    ] {
        let px = |v: f64| Fixed((v * 256.) as i32);
        let (left, top, right, bottom) = (40.3, 30.6, 200.9, 180.2);
        let radius = [radius, radius * 0.7, radius * 0.4, radius * 0.9];
        let shape = ShadowShape {
            left: px(left),
            top: px(top),
            right: px(right),
            bottom: px(bottom),
            radius: BorderRadius::new(px(radius[0]), px(radius[1]), px(radius[2]), px(radius[3])),
            weight: GAUSSIAN_ONE as i32,
        };
        let shadow = BoxShadowCommand::single(
            shape,
            px(sigma),
            white,
            Some(StraightRowProfile::new(240, px(left), px(right), Gaussian::new(px(sigma)))),
        );
        let f = |v: Fixed<i32, 8>| v.0 as f64 / 256.;
        let (left, top, right, bottom, sigma) =
            (f(shape.left), f(shape.top), f(shape.right), f(shape.bottom), f(shadow.sigma));
        let r = [
            f(shape.radius.top_left),
            f(shape.radius.top_right),
            f(shape.radius.bottom_right),
            f(shape.radius.bottom_left),
        ];
        let span = PhysicalRect::new(PhysicalPoint::new(0, 0), PhysicalSize::new(240, 220));
        // The rows within reach of the corners.
        for line in (20..80).chain(130..190) {
            let mut buffer = [PremultipliedRgbaColor::default(); 240];
            draw_box_shadow_line(&span, PhysicalLength::new(line), &shadow, &mut buffer, 0);
            let y_center = line as f64 + 0.5;
            // Rows of 1/16 px, each close to a rectangle.
            let rows = ((top.max(y_center - 4. * sigma) * 16.).floor() as i32)
                ..((bottom.min(y_center + 4. * sigma) * 16.).ceil() as i32);
            let rows: Vec<_> = rows
                .map(|row| {
                    let (y0, y1) =
                        ((row as f64 / 16.).max(top), ((row + 1) as f64 / 16.).min(bottom));
                    let weight = phi((y_center - y0) / sigma) - phi((y_center - y1) / sigma);
                    let middle = (y0 + y1) / 2.;
                    let x0 = left
                        + corner_inset(r[0], middle - top).max(corner_inset(r[3], bottom - middle));
                    let x1 = right
                        - corner_inset(r[1], middle - top).max(corner_inset(r[2], bottom - middle));
                    (weight, x0, x1)
                })
                .collect();
            for (x, pixel) in buffer.iter().enumerate() {
                let x_center = x as f64 + 0.5;
                let coverage: f64 = rows
                    .iter()
                    .map(|(w, x0, x1)| {
                        w * (phi((x_center - x0) / sigma) - phi((x_center - x1) / sigma))
                    })
                    .sum();
                let error = (pixel.alpha as f64 - coverage * 255.).abs();
                assert!(error <= 3.5, "sigma {sigma}, line {line}, x {x}: error {error}");
            }
        }
    }
}

#[test]
fn box_shadow_has_straight_rows_matches_a_row_scan() {
    let height = 60;
    // (top, bottom, top radius, bottom radius, sigma) in 1/256 px
    for (top, bottom, top_radius, bottom_radius, sigma) in [
        (10 * 256, 50 * 256, 0, 0, 4 * 256),
        (10 * 256 + 77, 50 * 256 + 3, 8 * 256, 5 * 256, 3 * 256 + 50),
        (10 * 256, 50 * 256, 16 * 256, 16 * 256, 2 * 256),
        (10 * 256, 50 * 256, 16 * 256, 16 * 256, 4 * 256),
        (-900 * 256, 50 * 256, 16 * 256, 0, 5 * 256),
        (10 * 256, 900 * 256, 0, 16 * 256, 5 * 256),
    ] {
        let shape = ShadowShape {
            left: Fixed(0),
            top: Fixed(top),
            right: Fixed(50 * 256),
            bottom: Fixed(bottom),
            radius: BorderRadius::new(
                Fixed(top_radius),
                Fixed(top_radius),
                Fixed(bottom_radius),
                Fixed(bottom_radius),
            ),
            weight: GAUSSIAN_ONE as i32,
        };
        let cutoff = Gaussian::new(Fixed(sigma)).cutoff();
        let (top_curve_end, bottom_curve_start) = shape.curve_bounds();
        let scanned = (0..height).any(|line| {
            let y_center = Fixed::from_integer(line) + HALF_PIXEL;
            shape.top.max(y_center - cutoff) >= top_curve_end
                && shape.bottom.min(y_center + cutoff) <= bottom_curve_start
        });
        assert_eq!(
            shape.has_straight_rows(Fixed(sigma), height),
            scanned,
            "top {top}, sigma {sigma}"
        );
    }
}

/// `options` with the opaque source of a plain `Rectangle` of its size and radius.
#[cfg(test)]
fn with_opaque_source(options: BoxShadowOptions) -> BoxShadowOptions {
    let size = euclid::size2(options.width.get(), options.height.get());
    let rect = euclid::Rect::from_size(size);
    let layout = i_slint_core::item_rendering::BorderRectLayout {
        brush_size: size,
        outer_radius: options.radius,
        background_rect: rect,
        background_radius: options.radius,
        border_rect: rect,
        border_radius: options.radius,
        border_width: Default::default(),
        border_color: Brush::default(),
    };
    BoxShadowOptions {
        source: Some((Brush::SolidColor(Color::from_rgb_u8(0, 0, 0)), layout)),
        ..options
    }
}

#[test]
fn drop_shadow_stays_within_bounding_rect() {
    use euclid::{point2, size2, vec2};
    use i_slint_core::lengths::RectLengths;
    let screen_size = PhysicalSize::new(120, 100);
    let geometry = LogicalRect::new(point2(30., 30.), size2(40., 20.));
    // (offset, blur, spread, radius) in logical pixels, and the scale factor
    for (offset, blur, spread, radius, scale_factor) in [
        (vec2(0., 0.), 10., 0., 0., 1.),
        (vec2(5.3, -3.7), 7.3, 4.6, 6., 1.5),
        (vec2(-2., 4.1), 5.1, -3.2, 20., 1.3),
        (vec2(3.3, 2.9), 0., 2.5, 8., 1.7),
    ] {
        for orientation in [
            RenderingRotation::NoRotation,
            RenderingRotation::Rotate90,
            RenderingRotation::Rotate180,
            RenderingRotation::Rotate270,
        ] {
            let rotation = RotationInfo { orientation, screen_size };
            let scale_factor = ScaleFactor::new(scale_factor);
            let options = with_opaque_source(BoxShadowOptions {
                width: geometry.width_length() * scale_factor,
                height: geometry.height_length() * scale_factor,
                color: Color::from_rgb_u8(255, 255, 255),
                blur: LogicalLength::new(blur) * scale_factor,
                radius: LogicalBorderRadius::new_uniform(radius) * scale_factor,
                spread: LogicalLength::new(spread) * scale_factor,
                ..Default::default()
            });

            let rotated_size = screen_size.transformed(rotation);
            let screen = PhysicalRect::from_size(rotated_size);
            let data = render_region(rotated_size, &[screen], scale_factor, |processor| {
                process_drop_shadow(
                    processor,
                    &options,
                    (geometry.origin + offset) * scale_factor,
                    options.color,
                    &screen,
                    rotation,
                )
            });

            // The dirty region the partial renderer uses for the shadow.
            let bounding_rect = (drop_shadow_bounding_rect(
                geometry,
                offset,
                LogicalLength::new(blur),
                LogicalLength::new(spread),
            ) * scale_factor)
                .round_out()
                .cast::<i16>()
                .transformed(rotation);
            let mut drawn = false;
            for (i, pixel) in data.iter().enumerate() {
                let p = PhysicalPoint::new(
                    (i % rotated_size.width as usize) as i16,
                    (i / rotated_size.width as usize) as i16,
                );
                drawn |= pixel.alpha > 0;
                assert!(
                    pixel.alpha == 0 || bounding_rect.contains(p),
                    "{p:?} outside {bounding_rect:?}, blur {blur}, spread {spread}, {orientation:?}"
                );
            }
            assert!(drawn, "nothing drawn, blur {blur}, spread {spread}, {orientation:?}");
        }
    }
}

#[test]
fn drop_shadow_partial_render_is_identical() {
    let options = |blur: f32, width: f32, height: f32, radius| {
        with_opaque_source(BoxShadowOptions {
            width: euclid::Length::new(width),
            height: euclid::Length::new(height),
            color: Color::from_argb_u8(200, 10, 20, 30),
            blur: euclid::Length::new(blur),
            radius,
            spread: euclid::Length::new(1.7),
            ..Default::default()
        })
    };
    // Each region cuts through the corner curves and the blur on every side.
    let cases = [
        (
            PhysicalSize::new(100, 80),
            options(9.3, 50., 30., BorderRadius::new(14., 3., 22., 0.)),
            euclid::point2(23.4, 21.8),
            None,
            [euclid::rect(13, 9, 31, 17), euclid::rect(52, 40, 29, 33)],
        ),
        // A blur large enough for the coverage to be interpolated between samples, with an
        // item clip that moves the origin of the drawn geometry.
        (
            PhysicalSize::new(360, 300),
            options(64.3, 170., 120., BorderRadius::new(34., 3., 52., 0.)),
            euclid::point2(100.4, 90.8),
            Some(euclid::rect(71, 57, 250, 200)),
            [euclid::rect(40, 30, 101, 57), euclid::rect(222, 180, 121, 103)],
        ),
        // Square corners, so every row is drawn from the straight-row profile,
        // with an item clip through its left ramp.
        (
            PhysicalSize::new(200, 160),
            options(12.2, 120., 90., BorderRadius::default()),
            euclid::point2(40.3, 30.6),
            Some(euclid::rect(50, 20, 140, 130)),
            [euclid::rect(30, 40, 40, 60), euclid::rect(140, 10, 50, 40)],
        ),
    ];
    for (screen_size, options, origin, clip, region) in cases {
        let clip = clip.unwrap_or(PhysicalRect::from_size(screen_size));
        assert_partial_render_is_identical(screen_size, clip, &region, |processor, clip| {
            process_drop_shadow(
                processor,
                &options,
                origin,
                options.color,
                clip,
                RotationInfo { orientation: RenderingRotation::NoRotation, screen_size },
            )
        });
    }
}

/// Renders the drop shadow of `options` at `origin` on a `screen_size` buffer.
#[cfg(test)]
fn render_drop_shadow(
    screen_size: PhysicalSize,
    options: &BoxShadowOptions,
    origin: euclid::Point2D<f32, PhysicalPx>,
) -> Vec<PremultipliedRgbaColor> {
    let screen = PhysicalRect::from_size(screen_size);
    render_region(screen_size, &[screen], ScaleFactor::new(1.), |processor| {
        process_drop_shadow(
            processor,
            options,
            origin,
            options.color,
            &screen,
            RotationInfo { orientation: RenderingRotation::NoRotation, screen_size },
        )
    })
}

#[test]
fn drop_shadow_with_sub_pixel_blur_is_anti_aliased() {
    let screen_size = PhysicalSize::new(40, 40);
    // How much of the pixel at `p` lies within `start..end`, along one axis.
    let overlap = |p: f32, start: f32, end: f32| (end.min(p + 1.) - start.max(p)).clamp(0., 1.);
    for fract in [0., 0.25, 0.5, 0.8] {
        let (x, y) = (10. + fract, 10. + fract / 2.);
        let options = with_opaque_source(BoxShadowOptions {
            width: euclid::Length::new(20.),
            height: euclid::Length::new(20.),
            color: Color::from_rgb_u8(255, 255, 255),
            blur: euclid::Length::new(0.01),
            ..Default::default()
        });
        let data = render_drop_shadow(screen_size, &options, euclid::point2(x, y));
        for (i, pixel) in data.iter().enumerate() {
            let (px, py) = ((i % 40) as f32, (i / 40) as f32);
            let expected = overlap(px, x, x + 20.) * overlap(py, y, y + 20.) * 255.;
            let near_edge = |p: f32, start: f32, end: f32| {
                (p + 0.5 - start).abs().min((p + 0.5 - end).abs()) < 1.5
            };
            // The Gaussian standing in for the pixel's box filter is off by up to 15/255 per
            // edge, compounding at a corner.
            let tolerance = match (near_edge(px, x, x + 20.), near_edge(py, y, y + 20.)) {
                (true, true) => 24.,
                (true, false) | (false, true) => 16.,
                (false, false) => 0.,
            };
            let difference = (pixel.alpha as f32 - expected).abs();
            assert!(
                difference <= tolerance,
                "fract {fract}, ({px}, {py}): {} vs {expected}",
                pixel.alpha
            );
        }
    }
}

#[test]
fn drop_shadow_of_huge_shape_matches_a_smaller_one() {
    let screen_size = PhysicalSize::new(60, 60);
    let options = |width, height, blur| {
        with_opaque_source(BoxShadowOptions {
            width: euclid::Length::new(width),
            height: euclid::Length::new(height),
            color: Color::from_rgb_u8(255, 255, 255),
            blur: euclid::Length::new(blur),
            radius: BorderRadius::new(0., 8., 0., 0.),
            ..Default::default()
        })
    };
    // Only the top right corner is within reach of the screen. The huge shape's left edge is
    // exactly representable, so both shapes share the same right edge.
    let render = |left: f32, height: f32| {
        render_drop_shadow(
            screen_size,
            &options(50. - left, height, 6.),
            euclid::point2(left, 20.5),
        )
    };
    let small = render(-500., 500.);
    let huge = render(-16777216., 16777216.);
    for (i, (a, b)) in small.iter().zip(huge.iter()).enumerate() {
        assert_eq!(bytemuck::bytes_of(a), bytemuck::bytes_of(b), "pixel {i}");
    }
    assert!(small.iter().any(|p| p.alpha > 0));

    let huge_blur =
        render_drop_shadow(screen_size, &options(3e7, 3e7, 3e7), euclid::point2(-1e7, -1e7));
    assert!(huge_blur.iter().any(|p| p.alpha > 0));
}
