// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains brush related types for the run-time library.
*/

use super::Color;
use crate::properties::InterpolatedPropertyValue;
use crate::SharedVector;
use euclid::default::{Point2D, Size2D};

#[cfg(not(feature = "std"))]
use num_traits::float::Float;
#[cfg(not(feature = "std"))]
use num_traits::Euclid;

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
            Brush::ConicGradient(g) => Brush::ConicGradient(ConicGradientBrush::new(
                g.from_angle,
                g.stops().map(|s| GradientStop {
                    color: s.color.brighter(factor),
                    position: s.position,
                }),
            )),
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
            Brush::ConicGradient(g) => Brush::ConicGradient(ConicGradientBrush::new(
                g.from_angle,
                g.stops()
                    .map(|s| GradientStop { color: s.color.darker(factor), position: s.position }),
            )),
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
            Brush::ConicGradient(g) => Brush::ConicGradient(ConicGradientBrush::new(
                g.from_angle,
                g.stops().map(|s| GradientStop {
                    color: s.color.transparentize(amount),
                    position: s.position,
                }),
            )),
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
            Brush::ConicGradient(g) => Brush::ConicGradient(ConicGradientBrush::new(
                g.from_angle,
                g.stops().map(|s| GradientStop {
                    color: s.color.with_alpha(alpha),
                    position: s.position,
                }),
            )),
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
#[repr(C)]
pub struct ConicGradientBrush {
    /// The starting angle (rotation) of the gradient in normalized form (0.0 = 0°, 1.0 = 360°)
    from_angle: f32,
    /// The color stops of the gradient
    stops: SharedVector<GradientStop>,
}

impl ConicGradientBrush {
    /// Creates a new conic gradient with the provided starting angle and color stops.
    ///
    /// The `from_angle` parameter is in normalized form (0.0 = 0°, 1.0 = 360°), corresponding
    /// to CSS's `from <angle>` syntax. It rotates the entire gradient clockwise.
    pub fn new(from_angle: f32, stops: impl IntoIterator<Item = GradientStop>) -> Self {
        let mut stops: alloc::vec::Vec<_> = stops.into_iter().collect();
        stops.sort_by(|a, b| {
            a.position.partial_cmp(&b.position).unwrap_or(core::cmp::Ordering::Equal)
        });

        // Add interpolated boundary stop at 0.0 if needed
        let has_stop_at_0 = stops.iter().any(|s| s.position.abs() < f32::EPSILON);
        if !has_stop_at_0 {
            // Find stops closest to boundaries for interpolation
            // For 0.0: find the stop just below 0 and just at/above 0
            let stop_below_0 = stops.iter().filter(|s| s.position < 0.0).last(); // closest to 0 from below
            let stop_above_0 = stops.iter().filter(|s| s.position >= 0.0).next(); // closest to 0 from above
            if let (Some(below), Some(above)) = (stop_below_0, stop_above_0) {
                // Interpolate between the stop below 0 and the stop above 0
                // Example: -10deg and 10deg → interpolate at 0deg
                let t = (0.0 - below.position) / (above.position - below.position);
                let color_at_0 = Self::interpolate_color(below.color, above.color, t);
                stops.insert(0, GradientStop { position: 0.0, color: color_at_0 });
            } else if let Some(above) = stop_above_0 {
                // Only stops above 0, use the first stop's color
                stops.insert(0, GradientStop { position: 0.0, color: above.color });
            }
        }

        // Add interpolated boundary stop at 1.0 if needed
        let has_stop_at_1 = stops.iter().any(|s| (s.position - 1.0).abs() < f32::EPSILON);
        if !has_stop_at_1 {
            // For 1.0: find the stop just at/below 1 and just above 1
            let stop_below_1 = stops.iter().filter(|s| s.position <= 1.0).last(); // closest to 1 from below
            let stop_above_1 = stops.iter().filter(|s| s.position > 1.0).next(); // closest to 1 from above

            if let (Some(below), Some(above)) = (stop_below_1, stop_above_1) {
                // Interpolate between the stop below 1 and the stop above 1
                // Example: 350deg and 370deg → interpolate at 360deg
                let t = (1.0 - below.position) / (above.position - below.position);
                let color_at_1 = Self::interpolate_color(below.color, above.color, t);
                stops.push(GradientStop { position: 1.0, color: color_at_1 });
            } else if let Some(below) = stop_below_1 {
                // Only stops below 1, use the last stop's color
                stops.push(GradientStop { position: 1.0, color: below.color });
            }
        }

        // Drop stops under 0deg and over 360deg
        stops = stops.into_iter().filter(|s| 0.0 <= s.position && s.position <= 1.0).collect();

        // Adjust first stop (at 0.0) to avoid duplicate with stop at 1.0
        if let Some(first) = stops.first_mut() {
            if first.position.abs() < f32::EPSILON {
                first.position = f32::EPSILON;
            }
        }

        Self { from_angle, stops: SharedVector::from_iter(stops.into_iter()) }
    }

    /// Returns the color stops of the conic gradient.
    pub fn stops(&self) -> impl Iterator<Item = &GradientStop> {
        self.stops.iter()
    }

    /// Returns the color stops with the `from_angle` rotation applied.
    ///
    /// This method returns a SharedVector of stops where:
    /// 1. Each stop's position is adjusted by adding `from_angle` and wrapping to [0.0, 1.0]
    /// 2. Duplicate positions with different colors are separated to avoid flickering
    /// 3. The stops are sorted by their rotated positions
    /// 4. Boundary stops at 0.0 and 1.0 are added if missing (with interpolated colors)
    ///
    /// If `from_angle` is effectively zero, this returns a clone of the internal stops
    /// without allocating a new vector.
    ///
    /// This is useful when you need to work with the actual visual positions of the stops
    /// after the gradient has been rotated.
    pub fn rotated_stops(&self) -> SharedVector<GradientStop> {
        let from_angle = self.from_angle - self.from_angle.floor();

        if from_angle.abs() <= f32::EPSILON {
            return self.stops.clone();
        }

        let mut stops: alloc::vec::Vec<_> = self.stops.iter().copied().collect();

        // Step 1: Apply rotation by adding from_angle and wrapping to [0, 1) range
        stops = stops
            .iter()
            .map(|stop| {
                #[cfg(feature = "std")]
                let rotated_position = (stop.position + from_angle).rem_euclid(1.0);
                #[cfg(not(feature = "std"))]
                let rotated_position = (stop.position + from_angle).rem_euclid(&1.0);
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
        if !has_stop_at_0 {
            // Find the color at position 0.0 by interpolating from the last and first stops
            if let (Some(last), Some(first)) = (stops.last(), stops.first()) {
                let gap = 1.0 - last.position + first.position;
                let color_at_0 = if gap > f32::EPSILON {
                    let t = (1.0 - last.position) / gap;
                    Self::interpolate_color(last.color, first.color, t)
                } else {
                    last.color
                };
                stops.insert(0, GradientStop { position: 0.0, color: color_at_0 });
            }
        }

        let has_stop_at_1 = stops.iter().any(|s| (s.position - 1.0).abs() < f32::EPSILON);
        if !has_stop_at_1 {
            // Add stop at 1.0 with same color as stop at 0.0
            if let Some(first) = stops.first() {
                stops.push(GradientStop { position: 1.0, color: first.color });
            }
        }

        SharedVector::from_iter(stops.into_iter())
    }

    /// Helper: Linearly interpolate between two colors
    fn interpolate_color(c1: Color, c2: Color, t: f32) -> Color {
        let argb1 = c1.to_argb_u8();
        let argb2 = c2.to_argb_u8();
        Color::from_argb_u8(
            ((1.0 - t) * argb1.alpha as f32 + t * argb2.alpha as f32) as u8,
            ((1.0 - t) * argb1.red as f32 + t * argb2.red as f32) as u8,
            ((1.0 - t) * argb1.green as f32 + t * argb2.green as f32) as u8,
            ((1.0 - t) * argb1.blue as f32 + t * argb2.blue as f32) as u8,
        )
    }
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

    if s > 0. {
        (a, b)
    } else {
        (b, a)
    }
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
                for x in new_grad.stops.make_mut_slice().iter_mut() {
                    x.color = col.interpolate(&x.color, t);
                }
                Brush::ConicGradient(new_grad)
            }
            (a @ Brush::ConicGradient(_), b @ Brush::SolidColor(_)) => {
                Self::interpolate(b, a, 1. - t)
            }
            (Brush::ConicGradient(lhs), Brush::ConicGradient(rhs)) => {
                if lhs.stops.len() < rhs.stops.len() {
                    Self::interpolate(target_value, self, 1. - t)
                } else {
                    let mut new_grad = lhs.clone();
                    new_grad.from_angle = lhs.from_angle.interpolate(&rhs.from_angle, t);
                    let mut iter = new_grad.stops.make_mut_slice().iter_mut();
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
