// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A Fixed point, represented with the T underlying type, and shifted by so many bits
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed<T, const SHIFT: usize>(pub T);

impl<
        T: Copy
            + core::ops::Shl<usize, Output = T>
            + core::ops::Shr<usize, Output = T>
            + core::ops::Div<Output = T>
            + core::ops::Add<Output = T>
            + core::ops::Rem<Output = T>,
        const SHIFT: usize,
    > Fixed<T, SHIFT>
{
    /// Create a fixed point from an integer value
    #[inline(always)]
    pub fn from_integer(value: T) -> Self {
        Self(value << SHIFT)
    }

    /// Get the integer part of the fixed point value
    #[inline(always)]
    pub fn truncate(self) -> T {
        self.0 >> SHIFT
    }

    /// Return the fractional part of the fixed point value
    #[inline(always)]
    pub fn fract(self) -> u8
    where
        T: num_traits::AsPrimitive<u8>,
    {
        if SHIFT < 8 {
            (self.0 >> (SHIFT - 8)).as_()
        } else {
            (self.0 << (8 - SHIFT)).as_()
        }
    }

    #[inline(always)]
    pub fn from_fixed<
        T2: core::ops::Shl<usize, Output = T2> + core::ops::Shr<usize, Output = T2> + Into<T>,
        const SHIFT2: usize,
    >(
        value: Fixed<T2, SHIFT2>,
    ) -> Self {
        if SHIFT > SHIFT2 {
            let s: T = value.0.into();
            Self(s << (SHIFT - SHIFT2))
        } else {
            Self((value.0 >> (SHIFT2 - SHIFT)).into())
        }
    }
    #[inline(always)]
    pub fn try_from_fixed<
        T2: core::ops::Shl<usize, Output = T2> + core::ops::Shr<usize, Output = T2> + TryInto<T>,
        const SHIFT2: usize,
    >(
        value: Fixed<T2, SHIFT2>,
    ) -> Result<Self, T2::Error> {
        Ok(if SHIFT > SHIFT2 {
            let s: T = value.0.try_into()?;
            Self(s << (SHIFT - SHIFT2))
        } else {
            Self((value.0 >> (SHIFT2 - SHIFT)).try_into()?)
        })
    }

    #[inline(always)]
    pub fn from_fraction(numerator: T, denominator: T) -> Self {
        Self((numerator << SHIFT) / denominator)
    }

    #[inline(always)]
    pub(crate) fn from_f32(value: f32) -> Option<Self>
    where
        T: num_traits::FromPrimitive,
    {
        Some(Self(T::from_f32(value * (1 << SHIFT) as f32)?))
    }
}

impl<T: core::ops::Add<Output = T>, const SHIFT: usize> core::ops::Add for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.add(rhs.0))
    }
}

impl<T: core::ops::Sub<Output = T>, const SHIFT: usize> core::ops::Sub for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.sub(rhs.0))
    }
}

impl<T: core::ops::AddAssign, const SHIFT: usize> core::ops::AddAssign for Fixed<T, SHIFT> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0.add_assign(rhs.0)
    }
}

impl<T: core::ops::SubAssign, const SHIFT: usize> core::ops::SubAssign for Fixed<T, SHIFT> {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0.sub_assign(rhs.0)
    }
}

impl<T: core::ops::Mul<Output = T>, const SHIFT: usize> core::ops::Mul<T> for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: T) -> Self::Output {
        Self(self.0.mul(rhs))
    }
}

impl<T: core::ops::Mul<Output = T>, const SHIFT: usize> core::ops::Mul<Fixed<T, SHIFT>>
    for Fixed<T, SHIFT>
where
    T: TryFrom<i64> + Into<i64>,
    <T as TryFrom<i64>>::Error: core::fmt::Debug,
{
    type Output = Self;
    fn mul(self, rhs: Fixed<T, SHIFT>) -> Self::Output {
        let lhs_i64: i64 = self.0.into();
        let rhs_i64: i64 = rhs.0.into();
        Self(T::try_from((lhs_i64 * rhs_i64) >> SHIFT).expect("attempt to multiply with overflow"))
    }
}

impl<T: core::ops::Neg<Output = T>, const SHIFT: usize> core::ops::Neg for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl<T: core::ops::Div<Output = T>, const SHIFT: usize> core::ops::Div for Fixed<T, SHIFT> {
    type Output = T;
    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl<T: core::ops::Rem<Output = T>, const SHIFT: usize> core::ops::Rem for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn rem(self, rhs: Self) -> Self::Output {
        Self(self.0 % rhs.0)
    }
}

impl<T: core::ops::Div<Output = T>, const SHIFT: usize> core::ops::Div<T> for Fixed<T, SHIFT> {
    type Output = Self;
    #[inline(always)]
    fn div(self, rhs: T) -> Self::Output {
        Self(self.0 / rhs)
    }
}
