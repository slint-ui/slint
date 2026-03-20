// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains brush related types for the run-time library.
*/

use super::Color;
use crate::SharedVector;
use crate::properties::InterpolatedPropertyValue;
use euclid::default::{Point2D, Size2D};

#[cfg(not(feature = "std"))]
use num_traits::Euclid;
#[cfg(not(feature = "std"))]
use num_traits::float::Float;

/// A brush is a data structure that is used to describe how
/// a shape, such as a rectangle, path or even text, shall be filled.
/// A brush can also be applied to the outline of a shape, that means
/// the fill of the outline itself.
#[derive(Clone, PartialEq, Debug, derive_more::From)]
#[repr(C)]
#[non_exhaustive]
pub enum Brush {
    /// The color variant of brush is a plain color that is to be used for the fill.
    SolidColor(Color),
    /// The linear gradient variant of a brush describes the gradient stops for a fill
    /// where all color stops are along a line that's rotated by the specified angle.
    LinearGradient(LinearGradientBrush),
    /// The radial gradient variant of a brush describes a circle variant centered
    /// in the middle
    RadialGradient(RadialGradientBrush),
    /// The conical gradient variant of a brush describes a gradient that rotates around
    /// a center point, like the hands of a clock
    ConicGradient(ConicGradientBrush),
}

/// Construct a brush with transparent color
impl Default for Brush {
    fn default() -> Self {
        Self::SolidColor(Color::default())
    }
}

impl Brush {
    /// If the brush is SolidColor, the contained color is returned.
    /// If the brush is a LinearGradient, the color of the first stop is returned.
    pub fn color(&self) -> Color {
        match self {
            Brush::SolidColor(col) => *col,
            Brush::LinearGradient(gradient) => {
                gradient.stops().next().map(|stop| stop.color).unwrap_or_default()
            }
            Brush::RadialGradient(gradient) => {
                gradient.stops().next().map(|stop| stop.color).unwrap_or_default()
            }
            Brush::ConicGradient(gradient) => {
                gradient.stops().next().map(|stop| stop.color).unwrap_or_default()
            }
        }
    }

    /// Returns true if this brush contains a fully transparent color (alpha value is zero)
    ///
    /// ```
    /// # use i_slint_core::graphics::*;
    /// assert!(Brush::default().is_transparent());
    /// assert!(Brush::SolidColor(Color::from_argb_u8(0, 255, 128, 140)).is_transparent());
    /// assert!(!Brush::SolidColor(Color::from_argb_u8(25, 128, 140, 210)).is_transparent());
    /// ```
    pub fn is_transparent(&self) -> bool {
        match self {
            Brush::SolidColor(c) => c.alpha() == 0,
            Brush::LinearGradient(_) => false,
            Brush::RadialGradient(_) => false,
            Brush::ConicGradient(_) => false,
        }
    }

    /// Returns true if this brush is fully opaque
    ///
    /// ```
    /// # use i_slint_core::graphics::*;
    /// assert!(!Brush::default().is_opaque());
    /// assert!(!Brush::SolidColor(Color::from_argb_u8(25, 255, 128, 140)).is_opaque());
    /// assert!(Brush::SolidColor(Color::from_rgb_u8(128, 140, 210)).is_opaque());
    /// ```
    pub fn is_opaque(&self) -> bool {
        match self {
            Brush::SolidColor(c) => c.alpha() == 255,
            Brush::LinearGradient(g) => g.stops().all(|s| s.color.alpha() == 255),
            Brush::RadialGradient(g) => g.stops().all(|s| s.color.alpha() == 255),
            Brush::ConicGradient(g) => g.stops().all(|s| s.color.alpha() == 255),
        }
    }

    /// Returns a new version of this brush that has the brightness increased
    /// by the specified factor. This is done by calling [`Color::brighter`] on
    /// all the colors of this brush.
    #[must_use]
    pub fn brighter(&self, factor: f32) -> Self {
        match self {
            Brush::SolidColor(c) => Brush::SolidColor(c.brighter(factor)),
            Brush::LinearGradient(g) => Brush::LinearGradient(LinearGradientBrush::new(
                g.angle(),
                g.stops().map(|s| GradientStop {
                    color: s.color.brighter(factor),
                    position: s.position,
                }),
            )),
            Brush::RadialGradient(g) => {
                Brush::RadialGradient(RadialGradientBrush::new_circle(g.stops().map(|s| {
                    GradientStop { color: s.color.brighter(factor), position: s.position }
                })))
            }
            Brush::ConicGradient(g) => {
                let mut new_grad = g.clone();
                // Skip the first stop (which contains the angle), modify only color stops
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = x.color.brighter(factor);
                }
                Brush::ConicGradient(new_grad)
            }
        }
    }

    /// Returns a new version of this brush that has the brightness decreased
    /// by the specified factor. This is done by calling [`Color::darker`] on
    /// all the color of this brush.
    #[must_use]
    pub fn darker(&self, factor: f32) -> Self {
        match self {
            Brush::SolidColor(c) => Brush::SolidColor(c.darker(factor)),
            Brush::LinearGradient(g) => Brush::LinearGradient(LinearGradientBrush::new(
                g.angle(),
                g.stops()
                    .map(|s| GradientStop { color: s.color.darker(factor), position: s.position }),
            )),
            Brush::RadialGradient(g) => Brush::RadialGradient(RadialGradientBrush::new_circle(
                g.stops()
                    .map(|s| GradientStop { color: s.color.darker(factor), position: s.position }),
            )),
            Brush::ConicGradient(g) => {
                let mut new_grad = g.clone();
                // Skip the first stop (which contains the angle), modify only color stops
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = x.color.darker(factor);
                }
                Brush::ConicGradient(new_grad)
            }
        }
    }

    /// Returns a new version of this brush with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    ///
    /// See also [`Color::transparentize`]
    #[must_use]
    pub fn transparentize(&self, amount: f32) -> Self {
        match self {
            Brush::SolidColor(c) => Brush::SolidColor(c.transparentize(amount)),
            Brush::LinearGradient(g) => Brush::LinearGradient(LinearGradientBrush::new(
                g.angle(),
                g.stops().map(|s| GradientStop {
                    color: s.color.transparentize(amount),
                    position: s.position,
                }),
            )),
            Brush::RadialGradient(g) => {
                Brush::RadialGradient(RadialGradientBrush::new_circle(g.stops().map(|s| {
                    GradientStop { color: s.color.transparentize(amount), position: s.position }
                })))
            }
            Brush::ConicGradient(g) => {
                let mut new_grad = g.clone();
                // Skip the first stop (which contains the angle), modify only color stops
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = x.color.transparentize(amount);
                }
                Brush::ConicGradient(new_grad)
            }
        }
    }

    /// Returns a new version of this brush with the related color's opacities
    /// set to `alpha`.
    #[must_use]
    pub fn with_alpha(&self, alpha: f32) -> Self {
        match self {
            Brush::SolidColor(c) => Brush::SolidColor(c.with_alpha(alpha)),
            Brush::LinearGradient(g) => Brush::LinearGradient(LinearGradientBrush::new(
                g.angle(),
                g.stops().map(|s| GradientStop {
                    color: s.color.with_alpha(alpha),
                    position: s.position,
                }),
            )),
            Brush::RadialGradient(g) => {
                Brush::RadialGradient(RadialGradientBrush::new_circle(g.stops().map(|s| {
                    GradientStop { color: s.color.with_alpha(alpha), position: s.position }
                })))
            }
            Brush::ConicGradient(g) => {
                let mut new_grad = g.clone();
                // Skip the first stop (which contains the angle), modify only color stops
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = x.color.with_alpha(alpha);
                }
                Brush::ConicGradient(new_grad)
            }
        }
    }
}

/// The LinearGradientBrush describes a way of filling a shape with different colors, which
/// are interpolated between different stops. The colors are aligned with a line that's rotated
/// by the LinearGradient's angle.
#[derive(Clone, PartialEq, Debug)]
#[repr(transparent)]
pub struct LinearGradientBrush(SharedVector<GradientStop>);

impl LinearGradientBrush {
    /// Creates a new linear gradient, described by the specified angle and the provided color stops.
    ///
    /// The angle need to be specified in degrees.
    /// The stops don't need to be sorted as this function will sort them.
    pub fn new(angle: f32, stops: impl IntoIterator<Item = GradientStop>) -> Self {
        let stop_iter = stops.into_iter();
        let mut encoded_angle_and_stops = SharedVector::with_capacity(stop_iter.size_hint().0 + 1);
        // The gradient's first stop is a fake stop to store the angle
        encoded_angle_and_stops.push(GradientStop { color: Default::default(), position: angle });
        encoded_angle_and_stops.extend(stop_iter);
        Self(encoded_angle_and_stops)
    }
    /// Returns the angle of the linear gradient in degrees.
    pub fn angle(&self) -> f32 {
        self.0[0].position
    }
    /// Returns the color stops of the linear gradient.
    /// The stops are sorted by positions.
    pub fn stops(&self) -> impl Iterator<Item = &GradientStop> {
        // skip the first fake stop that just contains the angle
        self.0.iter().skip(1)
    }
}

/// The RadialGradientBrush describes a way of filling a shape with a circular gradient
#[derive(Clone, PartialEq, Debug)]
#[repr(transparent)]
pub struct RadialGradientBrush(SharedVector<GradientStop>);

impl RadialGradientBrush {
    /// Creates a new circle radial gradient, centered in the middle and described
    /// by the provided color stops.
    pub fn new_circle(stops: impl IntoIterator<Item = GradientStop>) -> Self {
        Self(stops.into_iter().collect())
    }
    /// Returns the color stops of the linear gradient.
    pub fn stops(&self) -> impl Iterator<Item = &GradientStop> {
        self.0.iter()
    }
}

/// The ConicGradientBrush describes a way of filling a shape with a gradient
/// that rotates around a center point
#[derive(Clone, PartialEq, Debug)]
#[repr(transparent)]
pub struct ConicGradientBrush(SharedVector<GradientStop>);

impl ConicGradientBrush {
    /// Creates a new conic gradient, described by the specified angle and the provided color stops.
    ///
    /// The angle need to be specified in degrees (CSS `from <angle>` syntax).
    /// The stops don't need to be sorted as this function will normalize and process them.
    pub fn new(angle: f32, stops: impl IntoIterator<Item = GradientStop>) -> Self {
        let stop_iter = stops.into_iter();
        let mut encoded_angle_and_stops = SharedVector::with_capacity(stop_iter.size_hint().0 + 1);
        // The gradient's first stop is a fake stop to store the angle
        encoded_angle_and_stops.push(GradientStop { color: Default::default(), position: angle });
        encoded_angle_and_stops.extend(stop_iter);
        let mut result = Self(encoded_angle_and_stops);
        result.normalize_stops();
        if angle.abs() > f32::EPSILON {
            result.apply_rotation(angle);
        }
        result
    }

    /// Normalizes the gradient stops to be within [0, 1] range with proper boundary stops.
    fn normalize_stops(&mut self) {
        // Check if we need to make any changes
        let stops_slice = &self.0[1..];
        let has_stop_at_0 = stops_slice.iter().any(|s| s.position.abs() < f32::EPSILON);
        let has_stop_at_1 = stops_slice.iter().any(|s| (s.position - 1.0).abs() < f32::EPSILON);
        let has_stops_outside = stops_slice.iter().any(|s| s.position < 0.0 || s.position > 1.0);
        let is_empty = stops_slice.is_empty();

        // If no changes needed, return early
        if has_stop_at_0 && has_stop_at_1 && !has_stops_outside && !is_empty {
            return;
        }

        // Need to make changes, so copy
        let mut stops: alloc::vec::Vec<_> = stops_slice.to_vec();

        // Add interpolated boundary stop at 0.0 if needed
        if !has_stop_at_0 {
            let stop_below_0 = stops.iter().filter(|s| s.position < 0.0).max_by(|a, b| {
                a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
            });
            let stop_above_0 = stops.iter().filter(|s| s.position > 0.0).min_by(|a, b| {
                a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
            });
            if let (Some(below), Some(above)) = (stop_below_0, stop_above_0) {
                let t = (0.0 - below.position) / (above.position - below.position);
                let color_at_0 = Self::interpolate_color(&below.color, &above.color, t);
                stops.insert(0, GradientStop { position: 0.0, color: color_at_0 });
            } else if let Some(above) = stop_above_0 {
                stops.insert(0, GradientStop { position: 0.0, color: above.color });
            } else if let Some(below) = stop_below_0 {
                stops.insert(0, GradientStop { position: 0.0, color: below.color });
            }
        }

        // Add interpolated boundary stop at 1.0 if needed
        if !has_stop_at_1 {
            let stop_below_1 = stops.iter().filter(|s| s.position < 1.0).max_by(|a, b| {
                a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
            });
            let stop_above_1 = stops.iter().filter(|s| s.position > 1.0).min_by(|a, b| {
                a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
            });

            if let (Some(below), Some(above)) = (stop_below_1, stop_above_1) {
                let t = (1.0 - below.position) / (above.position - below.position);
                let color_at_1 = Self::interpolate_color(&below.color, &above.color, t);
                stops.push(GradientStop { position: 1.0, color: color_at_1 });
            } else if let Some(below) = stop_below_1 {
                stops.push(GradientStop { position: 1.0, color: below.color });
            } else if let Some(above) = stop_above_1 {
                stops.push(GradientStop { position: 1.0, color: above.color });
            }
        }

        // Drop stops outside [0, 1] range
        if has_stops_outside {
            stops.retain(|s| 0.0 <= s.position && s.position <= 1.0);
        }

        // Handle empty gradients
        if stops.is_empty() {
            stops.push(GradientStop { position: 0.0, color: Color::default() });
            stops.push(GradientStop { position: 1.0, color: Color::default() });
        }

        // Update the internal storage
        let angle = self.angle();
        self.0 = SharedVector::with_capacity(stops.len() + 1);
        self.0.push(GradientStop { color: Default::default(), position: angle });
        self.0.extend(stops);
    }

    /// Apply rotation to the gradient (CSS `from <angle>` syntax).
    ///
    /// The `from_angle` parameter is specified in degrees and rotates the entire gradient clockwise.
    fn apply_rotation(&mut self, from_angle: f32) {
        // Convert degrees to normalized 0-1 range
        let normalized_from_angle = (from_angle / 360.0) - (from_angle / 360.0).floor();

        // If no rotation needed, just update the stored angle
        if normalized_from_angle.abs() < f32::EPSILON {
            self.0.make_mut_slice()[0].position = from_angle;
            return;
        }

        // Update the stored angle
        self.0.make_mut_slice()[0].position = from_angle;

        // Need to rotate, so copy
        let mut stops: alloc::vec::Vec<_> = self.0.iter().skip(1).copied().collect();

        // Adjust first stop (at 0.0) to avoid duplicate with stop at 1.0
        if let Some(first) = stops.first_mut()
            && first.position.abs() < f32::EPSILON
        {
            first.position = f32::EPSILON;
        }

        // Step 1: Apply rotation by adding from_angle and wrapping to [0, 1) range
        stops = stops
            .iter()
            .map(|stop| {
                #[cfg(feature = "std")]
                let rotated_position = (stop.position + normalized_from_angle).rem_euclid(1.0);
                #[cfg(not(feature = "std"))]
                let rotated_position = (stop.position + normalized_from_angle).rem_euclid(&1.0);
                GradientStop { position: rotated_position, color: stop.color }
            })
            .collect();

        // Step 2: Separate duplicate positions with different colors to avoid flickering
        for i in 0..stops.len() {
            let j = (i + 1) % stops.len();
            if (stops[i].position - stops[j].position).abs() < f32::EPSILON
                && stops[i].color != stops[j].color
            {
                stops[i].position = (stops[i].position - f32::EPSILON).max(0.0);
                stops[j].position = (stops[j].position + f32::EPSILON).min(1.0);
            }
        }

        // Step 3: Sort by rotated position
        stops.sort_by(|a, b| {
            a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
        });

        // Step 4: Add boundary stops at 0.0 and 1.0 if missing
        let has_stop_at_0 = stops.iter().any(|s| s.position.abs() < f32::EPSILON);
        if !has_stop_at_0 && let (Some(last), Some(first)) = (stops.last(), stops.first()) {
            let gap = 1.0 - last.position + first.position;
            let color_at_0 = if gap > f32::EPSILON {
                let t = (1.0 - last.position) / gap;
                Self::interpolate_color(&last.color, &first.color, t)
            } else {
                last.color
            };
            stops.insert(0, GradientStop { position: 0.0, color: color_at_0 });
        }

        let has_stop_at_1 = stops.iter().any(|s| (s.position - 1.0).abs() < f32::EPSILON);
        if !has_stop_at_1 && let Some(first) = stops.first() {
            stops.push(GradientStop { position: 1.0, color: first.color });
        }

        // Update the internal storage
        self.0 = SharedVector::with_capacity(stops.len() + 1);
        self.0.push(GradientStop { color: Default::default(), position: from_angle });
        self.0.extend(stops);
    }

    /// Returns the starting angle (rotation) of the conic gradient in degrees.
    fn angle(&self) -> f32 {
        self.0[0].position
    }

    /// Returns the color stops of the conic gradient.
    /// The stops are already rotated according to the `from_angle` specified in `new()`.
    pub fn stops(&self) -> impl Iterator<Item = &GradientStop> {
        // skip the first fake stop that just contains the angle
        self.0.iter().skip(1)
    }

    /// Helper: Linearly interpolate between two colors using premultiplied alpha.
    ///
    /// This is used for interpolating gradient boundary colors in CSS-style gradients.
    /// We cannot use Color::mix() here because it implements Sass color mixing algorithm,
    /// which is different from CSS gradient color interpolation.
    ///
    /// CSS gradients interpolate in premultiplied RGBA space:
    /// https://www.w3.org/TR/css-color-4/#interpolation-alpha
    fn interpolate_color(c1: &Color, c2: &Color, factor: f32) -> Color {
        let argb1 = c1.to_argb_u8();
        let argb2 = c2.to_argb_u8();

        // Convert to premultiplied alpha
        let a1 = argb1.alpha as f32 / 255.0;
        let a2 = argb2.alpha as f32 / 255.0;
        let r1 = argb1.red as f32 * a1;
        let g1 = argb1.green as f32 * a1;
        let b1 = argb1.blue as f32 * a1;
        let r2 = argb2.red as f32 * a2;
        let g2 = argb2.green as f32 * a2;
        let b2 = argb2.blue as f32 * a2;

        // Interpolate in premultiplied space
        let alpha = (1.0 - factor) * a1 + factor * a2;
        let red = (1.0 - factor) * r1 + factor * r2;
        let green = (1.0 - factor) * g1 + factor * g2;
        let blue = (1.0 - factor) * b1 + factor * b2;

        // Convert back from premultiplied alpha
        if alpha > 0.0 {
            Color::from_argb_u8(
                (alpha * 255.0) as u8,
                (red / alpha).min(255.0) as u8,
                (green / alpha).min(255.0) as u8,
                (blue / alpha).min(255.0) as u8,
            )
        } else {
            Color::from_argb_u8(0, 0, 0, 0)
        }
    }
}

/// C FFI function to normalize the gradient stops to be within [0, 1] range
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub extern "C" fn slint_conic_gradient_normalize_stops(gradient: &mut ConicGradientBrush) {
    gradient.normalize_stops();
}

/// C FFI function to apply rotation to a ConicGradientBrush
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub extern "C" fn slint_conic_gradient_apply_rotation(
    gradient: &mut ConicGradientBrush,
    angle_degrees: f32,
) {
    gradient.apply_rotation(angle_degrees);
}

/// GradientStop describes a single color stop in a gradient. The colors between multiple
/// stops are interpolated.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GradientStop {
    /// The color to draw at this stop.
    pub color: Color,
    /// The position of this stop on the entire shape, as a normalized value between 0 and 1.
    pub position: f32,
}

/// Returns the start / end points of a gradient within a rectangle of the given size, based on the angle (in degree).
pub fn line_for_angle(angle: f32, size: Size2D<f32>) -> (Point2D<f32>, Point2D<f32>) {
    let angle = (angle + 90.).to_radians();
    let (s, c) = angle.sin_cos();

    let (a, b) = if s.abs() < f32::EPSILON {
        let y = size.height / 2.;
        return if c < 0. {
            (Point2D::new(0., y), Point2D::new(size.width, y))
        } else {
            (Point2D::new(size.width, y), Point2D::new(0., y))
        };
    } else if c * s < 0. {
        // Intersection between the gradient line, and an orthogonal line that goes through (height, 0)
        let x = (s * size.width + c * size.height) * s / 2.;
        let y = -c * x / s + size.height;
        (Point2D::new(x, y), Point2D::new(size.width - x, size.height - y))
    } else {
        // Intersection between the gradient line, and an orthogonal line that goes through (0, 0)
        let x = (s * size.width - c * size.height) * s / 2.;
        let y = -c * x / s;
        (Point2D::new(size.width - x, size.height - y), Point2D::new(x, y))
    };

    if s > 0. { (a, b) } else { (b, a) }
}

impl InterpolatedPropertyValue for Brush {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        match (self, target_value) {
            (Brush::SolidColor(source_col), Brush::SolidColor(target_col)) => {
                Brush::SolidColor(source_col.interpolate(target_col, t))
            }
            (Brush::SolidColor(col), Brush::LinearGradient(grad)) => {
                let mut new_grad = grad.clone();
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = col.interpolate(&x.color, t);
                }
                Brush::LinearGradient(new_grad)
            }
            (a @ Brush::LinearGradient(_), b @ Brush::SolidColor(_)) => {
                Self::interpolate(b, a, 1. - t)
            }
            (Brush::LinearGradient(lhs), Brush::LinearGradient(rhs)) => {
                if lhs.0.len() < rhs.0.len() {
                    Self::interpolate(target_value, self, 1. - t)
                } else {
                    let mut new_grad = lhs.clone();
                    let mut iter = new_grad.0.make_mut_slice().iter_mut();
                    {
                        let angle = &mut iter.next().unwrap().position;
                        *angle = angle.interpolate(&rhs.angle(), t);
                    }
                    for s2 in rhs.stops() {
                        let s1 = iter.next().unwrap();
                        s1.color = s1.color.interpolate(&s2.color, t);
                        s1.position = s1.position.interpolate(&s2.position, t);
                    }
                    for x in iter {
                        x.position = x.position.interpolate(&1.0, t);
                    }
                    Brush::LinearGradient(new_grad)
                }
            }
            (Brush::SolidColor(col), Brush::RadialGradient(grad)) => {
                let mut new_grad = grad.clone();
                for x in new_grad.0.make_mut_slice().iter_mut() {
                    x.color = col.interpolate(&x.color, t);
                }
                Brush::RadialGradient(new_grad)
            }
            (a @ Brush::RadialGradient(_), b @ Brush::SolidColor(_)) => {
                Self::interpolate(b, a, 1. - t)
            }
            (Brush::RadialGradient(lhs), Brush::RadialGradient(rhs)) => {
                if lhs.0.len() < rhs.0.len() {
                    Self::interpolate(target_value, self, 1. - t)
                } else {
                    let mut new_grad = lhs.clone();
                    let mut iter = new_grad.0.make_mut_slice().iter_mut();
                    let mut last_color = Color::default();
                    for s2 in rhs.stops() {
                        let s1 = iter.next().unwrap();
                        last_color = s2.color;
                        s1.color = s1.color.interpolate(&s2.color, t);
                        s1.position = s1.position.interpolate(&s2.position, t);
                    }
                    for x in iter {
                        x.position = x.position.interpolate(&1.0, t);
                        x.color = x.color.interpolate(&last_color, t);
                    }
                    Brush::RadialGradient(new_grad)
                }
            }
            (Brush::SolidColor(col), Brush::ConicGradient(grad)) => {
                let mut new_grad = grad.clone();
                for x in new_grad.0.make_mut_slice().iter_mut().skip(1) {
                    x.color = col.interpolate(&x.color, t);
                }
                Brush::ConicGradient(new_grad)
            }
            (a @ Brush::ConicGradient(_), b @ Brush::SolidColor(_)) => {
                Self::interpolate(b, a, 1. - t)
            }
            (Brush::ConicGradient(lhs), Brush::ConicGradient(rhs)) => {
                if lhs.0.len() < rhs.0.len() {
                    Self::interpolate(target_value, self, 1. - t)
                } else {
                    let mut new_grad = lhs.clone();
                    let mut iter = new_grad.0.make_mut_slice().iter_mut();
                    {
                        let angle = &mut iter.next().unwrap().position;
                        *angle = angle.interpolate(&rhs.angle(), t);
                    }
                    for s2 in rhs.stops() {
                        let s1 = iter.next().unwrap();
                        s1.color = s1.color.interpolate(&s2.color, t);
                        s1.position = s1.position.interpolate(&s2.position, t);
                    }
                    for x in iter {
                        x.position = x.position.interpolate(&1.0, t);
                    }
                    Brush::ConicGradient(new_grad)
                }
            }
            (a @ Brush::LinearGradient(_), b @ Brush::RadialGradient(_))
            | (a @ Brush::RadialGradient(_), b @ Brush::LinearGradient(_))
            | (a @ Brush::LinearGradient(_), b @ Brush::ConicGradient(_))
            | (a @ Brush::ConicGradient(_), b @ Brush::LinearGradient(_))
            | (a @ Brush::RadialGradient(_), b @ Brush::ConicGradient(_))
            | (a @ Brush::ConicGradient(_), b @ Brush::RadialGradient(_)) => {
                // Just go to an intermediate color.
                let color = Color::interpolate(&b.color(), &a.color(), t);
                if t < 0.5 {
                    Self::interpolate(a, &Brush::SolidColor(color), t * 2.)
                } else {
                    Self::interpolate(&Brush::SolidColor(color), b, (t - 0.5) * 2.)
                }
            }
        }
    }
}

#[test]
#[allow(clippy::float_cmp)] // We want bit-wise equality here
fn test_linear_gradient_encoding() {
    let stops: SharedVector<GradientStop> = [
        GradientStop { position: 0.0, color: Color::from_argb_u8(255, 255, 0, 0) },
        GradientStop { position: 0.5, color: Color::from_argb_u8(255, 0, 255, 0) },
        GradientStop { position: 1.0, color: Color::from_argb_u8(255, 0, 0, 255) },
    ]
    .into();
    let grad = LinearGradientBrush::new(256., stops.clone());
    assert_eq!(grad.angle(), 256.);
    assert!(grad.stops().eq(stops.iter()));
}

#[test]
fn test_conic_gradient_basic() {
    // Test basic conic gradient with no rotation
    let grad = ConicGradientBrush::new(
        0.0,
        [
            GradientStop { position: 0.0, color: Color::from_rgb_u8(255, 0, 0) },
            GradientStop { position: 0.5, color: Color::from_rgb_u8(0, 255, 0) },
            GradientStop { position: 1.0, color: Color::from_rgb_u8(255, 0, 0) },
        ],
    );
    assert_eq!(grad.angle(), 0.0);
    assert_eq!(grad.stops().count(), 3);
}

#[test]
fn test_conic_gradient_with_rotation() {
    // Test conic gradient with 90 degree rotation
    let grad = ConicGradientBrush::new(
        90.0,
        [
            GradientStop { position: 0.0, color: Color::from_rgb_u8(255, 0, 0) },
            GradientStop { position: 1.0, color: Color::from_rgb_u8(255, 0, 0) },
        ],
    );
    assert_eq!(grad.angle(), 90.0);
    // After rotation, stops should still be present and sorted
    assert!(grad.stops().count() >= 2);
}

#[test]
fn test_conic_gradient_negative_angle() {
    // Test with negative angle - should be normalized
    let grad = ConicGradientBrush::new(
        -90.0,
        [GradientStop { position: 0.5, color: Color::from_rgb_u8(255, 0, 0) }],
    );
    assert_eq!(grad.angle(), -90.0); // Angle is stored as-is
    assert!(grad.stops().count() >= 2); // Should have boundary stops added
}

#[test]
fn test_conic_gradient_stops_outside_range() {
    // Test with stops outside [0, 1] range
    let grad = ConicGradientBrush::new(
        0.0,
        [
            GradientStop { position: -0.2, color: Color::from_rgb_u8(255, 0, 0) },
            GradientStop { position: 0.5, color: Color::from_rgb_u8(0, 255, 0) },
            GradientStop { position: 1.2, color: Color::from_rgb_u8(0, 0, 255) },
        ],
    );
    // All stops should be within [0, 1] after processing
    for stop in grad.stops() {
        assert!(stop.position >= 0.0 && stop.position <= 1.0);
    }
}

#[test]
fn test_conic_gradient_all_stops_below_zero() {
    // Test edge case: all stops are below 0
    let grad = ConicGradientBrush::new(
        0.0,
        [
            GradientStop { position: -0.5, color: Color::from_rgb_u8(255, 0, 0) },
            GradientStop { position: -0.3, color: Color::from_rgb_u8(0, 255, 0) },
        ],
    );
    // Should create valid boundary stops
    assert!(grad.stops().count() >= 2);
    // First stop should be at or near 0.0
    let first = grad.stops().next().unwrap();
    assert!(first.position >= 0.0 && first.position < 0.1);
}

#[test]
fn test_conic_gradient_all_stops_above_one() {
    // Test edge case: all stops are above 1
    let grad = ConicGradientBrush::new(
        0.0,
        [
            GradientStop { position: 1.2, color: Color::from_rgb_u8(255, 0, 0) },
            GradientStop { position: 1.5, color: Color::from_rgb_u8(0, 255, 0) },
        ],
    );
    // Should create valid boundary stops
    assert!(grad.stops().count() >= 2);
    // Last stop should be at or near 1.0
    let last = grad.stops().last().unwrap();
    assert!(last.position > 0.9 && last.position <= 1.0);
}

#[test]
fn test_conic_gradient_empty() {
    // Test edge case: no stops provided
    let grad = ConicGradientBrush::new(0.0, []);
    // Should create default transparent stops
    assert_eq!(grad.stops().count(), 2);
}
