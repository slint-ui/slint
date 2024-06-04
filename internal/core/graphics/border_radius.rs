// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains border radius related types for the run-time library.
*/

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use euclid::approxord::{max, min};
use euclid::num::Zero;
use euclid::{Length, Scale};
use num_traits::NumCast;

/// Top-left, top-right, bottom-right, and bottom-left border radius, optionally
/// tagged with a unit.
#[repr(C)]
pub struct BorderRadius<T, U> {
    /// The top-left radius.
    pub top_left: T,
    /// The top-right radius.
    pub top_right: T,
    /// The bottom-right radius.
    pub bottom_right: T,
    /// The bottom-left radius.
    pub bottom_left: T,
    #[doc(hidden)]
    pub _unit: PhantomData<U>,
}

impl<T, U> Copy for BorderRadius<T, U> where T: Copy {}

impl<T, U> Clone for BorderRadius<T, U>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        BorderRadius {
            top_left: self.top_left.clone(),
            top_right: self.top_right.clone(),
            bottom_right: self.bottom_right.clone(),
            bottom_left: self.bottom_left.clone(),
            _unit: PhantomData,
        }
    }
}

impl<T, U> Eq for BorderRadius<T, U> where T: Eq {}

impl<T, U> PartialEq for BorderRadius<T, U>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.top_left == other.top_left
            && self.top_right == other.top_right
            && self.bottom_right == other.bottom_right
            && self.bottom_left == other.bottom_left
    }
}

impl<T, U> fmt::Debug for BorderRadius<T, U>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BorderRadius({:?}, {:?}, {:?}, {:?})",
            self.top_left, self.top_right, self.bottom_right, self.bottom_left
        )
    }
}

impl<T, U> Default for BorderRadius<T, U>
where
    T: Default,
{
    fn default() -> Self {
        BorderRadius::new(T::default(), T::default(), T::default(), T::default())
    }
}

impl<T, U> Zero for BorderRadius<T, U>
where
    T: Zero,
{
    fn zero() -> Self {
        BorderRadius::new(T::zero(), T::zero(), T::zero(), T::zero())
    }
}

impl<T, U> BorderRadius<T, U> {
    /// Constructor taking a scalar for each radius.
    ///
    /// Radii are specified in top-left, top-right, bottom-right, bottom-left
    /// order following CSS's convention.
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        BorderRadius { top_left, top_right, bottom_right, bottom_left, _unit: PhantomData }
    }

    /// Constructor taking a typed Length for each radius.
    ///
    /// Radii are specified in top-left, top-right, bottom-right, bottom-left
    /// order following CSS's convention.
    pub fn from_lengths(
        top_left: Length<T, U>,
        top_right: Length<T, U>,
        bottom_right: Length<T, U>,
        bottom_left: Length<T, U>,
    ) -> Self {
        BorderRadius::new(top_left.0, top_right.0, bottom_right.0, bottom_left.0)
    }

    /// Constructor taking the same scalar value for all radii.
    pub fn new_uniform(all: T) -> Self
    where
        T: Copy,
    {
        BorderRadius::new(all, all, all, all)
    }

    /// Constructor taking the same typed Length for all radii.
    pub fn from_length(all: Length<T, U>) -> Self
    where
        T: Copy,
    {
        BorderRadius::new_uniform(all.0)
    }

    /// Returns `true` if all radii are equal.
    pub fn is_uniform(&self) -> bool
    where
        T: ApproxEq<T>,
    {
        self.top_left.approx_eq(&self.top_right)
            && self.top_left.approx_eq(&self.bottom_right)
            && self.top_left.approx_eq(&self.bottom_left)
    }

    /// Returns the uniform radius if all are equal, or `None` otherwise.
    pub fn as_uniform(&self) -> Option<T>
    where
        T: Copy + ApproxEq<T>,
    {
        if self.is_uniform() {
            Some(self.top_left)
        } else {
            None
        }
    }

    /// Returns `true` if all radii are zero.
    pub fn is_zero(&self) -> bool
    where
        T: ApproxEq<T> + Zero,
    {
        let zero = T::zero();
        self.top_left.approx_eq(&zero)
            && self.top_right.approx_eq(&zero)
            && self.bottom_right.approx_eq(&zero)
            && self.bottom_left.approx_eq(&zero)
    }

    /// Returns the outer radius.
    ///
    /// For any corner with a positive radius, the radius is ensured to be at
    /// least `half_border_width`.
    pub fn outer(&self, half_border_width: Length<T, U>) -> Self
    where
        T: Copy + PartialOrd + Zero,
    {
        let zero = T::zero();
        BorderRadius::new(
            if self.top_left > zero {
                max(self.top_left, half_border_width.0)
            } else {
                self.top_left
            },
            if self.top_right > zero {
                max(self.top_right, half_border_width.0)
            } else {
                self.top_right
            },
            if self.bottom_right > zero {
                max(self.bottom_right, half_border_width.0)
            } else {
                self.bottom_right
            },
            if self.bottom_left > zero {
                max(self.bottom_left, half_border_width.0)
            } else {
                self.bottom_left
            },
        )
    }

    /// Returns the inner radius.
    ///
    /// A positive radius of each corner is subtracted by `half_border_width`
    /// and min-clamped to zero.
    pub fn inner(&self, half_border_width: Length<T, U>) -> Self
    where
        T: Copy + PartialOrd + Sub<T, Output = T> + Zero,
    {
        BorderRadius::new(
            self.top_left - half_border_width.0,
            self.top_right - half_border_width.0,
            self.bottom_right - half_border_width.0,
            self.bottom_left - half_border_width.0,
        )
        .max(Self::zero())
    }
}

/// Trait for testing approximate equality
pub trait ApproxEq<Eps> {
    /// Returns `true` is this object is approximately equal to the other one.
    fn approx_eq(&self, other: &Self) -> bool;
}

macro_rules! approx_eq {
    ($ty:ty, $eps:expr) => {
        impl ApproxEq<$ty> for $ty {
            #[inline]
            fn approx_eq(&self, other: &$ty) -> bool {
                num_traits::sign::abs(*self - *other) <= $eps
            }
        }
    };
}

approx_eq!(i16, 0);
approx_eq!(i32, 0);
approx_eq!(f32, f32::EPSILON);

impl<T, U> Add for BorderRadius<T, U>
where
    T: Add<T, Output = T>,
{
    type Output = Self;
    fn add(self, other: Self) -> Self {
        BorderRadius::new(
            self.top_left + other.top_left,
            self.top_right + other.top_right,
            self.bottom_right + other.bottom_right,
            self.bottom_left + other.bottom_left,
        )
    }
}

impl<T, U> AddAssign<Self> for BorderRadius<T, U>
where
    T: AddAssign<T>,
{
    fn add_assign(&mut self, other: Self) {
        self.top_left += other.top_left;
        self.top_right += other.top_right;
        self.bottom_right += other.bottom_right;
        self.bottom_left += other.bottom_left;
    }
}

impl<T, U> Sub for BorderRadius<T, U>
where
    T: Sub<T, Output = T>,
{
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        BorderRadius::new(
            self.top_left - other.top_left,
            self.top_right - other.top_right,
            self.bottom_right - other.bottom_right,
            self.bottom_left - other.bottom_left,
        )
    }
}

impl<T, U> SubAssign<Self> for BorderRadius<T, U>
where
    T: SubAssign<T>,
{
    fn sub_assign(&mut self, other: Self) {
        self.top_left -= other.top_left;
        self.top_right -= other.top_right;
        self.bottom_right -= other.bottom_right;
        self.bottom_left -= other.bottom_left;
    }
}

impl<T, U> Neg for BorderRadius<T, U>
where
    T: Neg<Output = T>,
{
    type Output = Self;
    fn neg(self) -> Self {
        BorderRadius {
            top_left: -self.top_left,
            top_right: -self.top_right,
            bottom_right: -self.bottom_right,
            bottom_left: -self.bottom_left,
            _unit: PhantomData,
        }
    }
}

impl<T, U> Mul<T> for BorderRadius<T, U>
where
    T: Copy + Mul,
{
    type Output = BorderRadius<T::Output, U>;

    #[inline]
    fn mul(self, scale: T) -> Self::Output {
        BorderRadius::new(
            self.top_left * scale,
            self.top_right * scale,
            self.bottom_right * scale,
            self.bottom_left * scale,
        )
    }
}

impl<T, U> MulAssign<T> for BorderRadius<T, U>
where
    T: Copy + MulAssign,
{
    #[inline]
    fn mul_assign(&mut self, other: T) {
        self.top_left *= other;
        self.top_right *= other;
        self.bottom_right *= other;
        self.bottom_left *= other;
    }
}

impl<T, U1, U2> Mul<Scale<T, U1, U2>> for BorderRadius<T, U1>
where
    T: Copy + Mul,
{
    type Output = BorderRadius<T::Output, U2>;

    #[inline]
    fn mul(self, scale: Scale<T, U1, U2>) -> Self::Output {
        BorderRadius::new(
            self.top_left * scale.0,
            self.top_right * scale.0,
            self.bottom_right * scale.0,
            self.bottom_left * scale.0,
        )
    }
}

impl<T, U> MulAssign<Scale<T, U, U>> for BorderRadius<T, U>
where
    T: Copy + MulAssign,
{
    #[inline]
    fn mul_assign(&mut self, other: Scale<T, U, U>) {
        *self *= other.0;
    }
}

impl<T, U> Div<T> for BorderRadius<T, U>
where
    T: Copy + Div,
{
    type Output = BorderRadius<T::Output, U>;

    #[inline]
    fn div(self, scale: T) -> Self::Output {
        BorderRadius::new(
            self.top_left / scale,
            self.top_right / scale,
            self.bottom_right / scale,
            self.bottom_left / scale,
        )
    }
}

impl<T, U> DivAssign<T> for BorderRadius<T, U>
where
    T: Copy + DivAssign,
{
    #[inline]
    fn div_assign(&mut self, other: T) {
        self.top_left /= other;
        self.top_right /= other;
        self.bottom_right /= other;
        self.bottom_left /= other;
    }
}

impl<T, U1, U2> Div<Scale<T, U1, U2>> for BorderRadius<T, U2>
where
    T: Copy + Div,
{
    type Output = BorderRadius<T::Output, U1>;

    #[inline]
    fn div(self, scale: Scale<T, U1, U2>) -> Self::Output {
        BorderRadius::new(
            self.top_left / scale.0,
            self.top_right / scale.0,
            self.bottom_right / scale.0,
            self.bottom_left / scale.0,
        )
    }
}

impl<T, U> DivAssign<Scale<T, U, U>> for BorderRadius<T, U>
where
    T: Copy + DivAssign,
{
    fn div_assign(&mut self, other: Scale<T, U, U>) {
        *self /= other.0;
    }
}

impl<T, U> BorderRadius<T, U>
where
    T: PartialOrd,
{
    /// Returns the minimum of the two radii.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        BorderRadius::new(
            min(self.top_left, other.top_left),
            min(self.top_right, other.top_right),
            min(self.bottom_right, other.bottom_right),
            min(self.bottom_left, other.bottom_left),
        )
    }

    /// Returns the maximum of the two radii.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        BorderRadius::new(
            max(self.top_left, other.top_left),
            max(self.top_right, other.top_right),
            max(self.bottom_right, other.bottom_right),
            max(self.bottom_left, other.bottom_left),
        )
    }
}

impl<T, U> BorderRadius<T, U>
where
    T: NumCast + Copy,
{
    /// Cast from one numeric representation to another, preserving the units.
    #[inline]
    pub fn cast<NewT: NumCast>(self) -> BorderRadius<NewT, U> {
        self.try_cast().unwrap()
    }

    /// Fallible cast from one numeric representation to another, preserving the units.
    pub fn try_cast<NewT: NumCast>(self) -> Option<BorderRadius<NewT, U>> {
        match (
            NumCast::from(self.top_left),
            NumCast::from(self.top_right),
            NumCast::from(self.bottom_right),
            NumCast::from(self.bottom_left),
        ) {
            (Some(top_left), Some(top_right), Some(bottom_right), Some(bottom_left)) => {
                Some(BorderRadius::new(top_left, top_right, bottom_right, bottom_left))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lengths::{LogicalBorderRadius, LogicalLength, PhysicalPx, ScaleFactor};
    use euclid::UnknownUnit;

    type BorderRadius = super::BorderRadius<f32, UnknownUnit>;
    type IntBorderRadius = super::BorderRadius<i16, UnknownUnit>;
    type PhysicalBorderRadius = super::BorderRadius<f32, PhysicalPx>;

    #[test]
    fn test_eq() {
        let a = BorderRadius::new(1., 2., 3., 4.);
        let b = BorderRadius::new(1., 2., 3., 4.);
        let c = BorderRadius::new(4., 3., 2., 1.);
        let d = BorderRadius::new(
            c.top_left + f32::EPSILON / 2.,
            c.top_right - f32::EPSILON / 2.,
            c.bottom_right - f32::EPSILON / 2.,
            c.bottom_left + f32::EPSILON / 2.,
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(c, d);
    }

    #[test]
    fn test_min_max() {
        let a = BorderRadius::new(1., 2., 3., 4.);
        let b = BorderRadius::new(4., 3., 2., 1.);
        assert_eq!(a.min(b), BorderRadius::new(1., 2., 2., 1.));
        assert_eq!(a.max(b), BorderRadius::new(4., 3., 3., 4.));
    }

    #[test]
    fn test_scale() {
        let scale = ScaleFactor::new(2.);
        let logical_radius = LogicalBorderRadius::new(1., 2., 3., 4.);
        let physical_radius = PhysicalBorderRadius::new(2., 4., 6., 8.);
        assert_eq!(logical_radius * scale, physical_radius);
        assert_eq!(physical_radius / scale, logical_radius);
    }

    #[test]
    fn test_zero() {
        assert!(BorderRadius::new_uniform(0.).is_zero());
        assert!(BorderRadius::new_uniform(1.0e-9).is_zero());
        assert!(!BorderRadius::new_uniform(1.0e-3).is_zero());
        assert!(IntBorderRadius::new_uniform(0).is_zero());
        assert!(!IntBorderRadius::new_uniform(1).is_zero());
    }

    #[test]
    fn test_inner_outer() {
        let radius = LogicalBorderRadius::new(0., 2.5, 5., 10.);
        let half_border_width = LogicalLength::new(5.);
        assert_eq!(radius.inner(half_border_width), LogicalBorderRadius::new(0., 0., 0., 5.));
        assert_eq!(radius.outer(half_border_width), LogicalBorderRadius::new(0., 5., 5., 10.));
    }
}
