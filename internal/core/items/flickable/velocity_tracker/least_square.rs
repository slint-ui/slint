// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Least-squares polynomial fitting via Gram-Schmidt QR decomposition, used
//! to estimate flick velocity from a short history of pointer samples.
//!
//! Ported from Flutter's `LeastSquaresSolver` (lsq_solver.dart), which is:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/gestures/lsq_solver.dart>
//!
//! Changes to the original:
//!     - no need to have generic weights (all are one for the scrolling in flutter and anywhere else yet used)
//!     - generic over the float type
//!
//! [`LeastSquaresSolver::solve`] takes the polynomial `degree` as a runtime
//! argument, matching the original API, rather than as a const generic:
//! stable Rust has no way to turn a const generic `DEGREE` into a `DEGREE +
//! 1`-sized array for the coefficients. Instead, `MAX_COEFFS` is a separate,
//! compile-time upper bound on how many coefficients (`degree + 1`) a fit
//! can ever produce, used only to size the fixed storage.

use core::iter::Sum;
use core::ops::{AddAssign, DivAssign, MulAssign, SubAssign};
use num_traits::{Float, NumCast, One, Zero};

/// Values at or below this magnitude are treated as zero when checking for a
/// degenerate (linearly dependent) fit.
const PRECISION_ERROR_TOLERANCE: f64 = 1e-10;

fn dot<T: Float + Sum>(a: &[T], b: &[T]) -> T {
    a.iter().zip(b).map(|(l, r)| *l * *r).sum()
}

fn norm<T: Float + Sum>(v: &[T]) -> T {
    dot(v, v).sqrt()
}

/// Row-major matrix backed by a fixed-size array.
///
/// `ROWS` is fixed at compile time. `MAX_COLS` only bounds the storage;
/// the number of columns actually in use (`columns`) is chosen at
/// construction time and can be smaller, since callers may have fewer
/// samples than the maximum they're prepared to store. This keeps the
/// Gram-Schmidt process below allocation-free.
struct Matrix<T, const ROWS: usize, const MAX_COLS: usize> {
    columns: usize,
    elements: [[T; MAX_COLS]; ROWS],
}

impl<T: Float, const ROWS: usize, const MAX_COLS: usize> Matrix<T, ROWS, MAX_COLS> {
    fn new(columns: usize) -> Self {
        debug_assert!(columns <= MAX_COLS);
        Self { columns, elements: [[T::zero(); MAX_COLS]; ROWS] }
    }

    fn get(&self, row: usize, col: usize) -> T {
        self.elements[row][col]
    }

    fn set(&mut self, row: usize, col: usize, value: T) {
        self.elements[row][col] = value;
    }

    fn row(&self, row: usize) -> &[T] {
        &self.elements[row][..self.columns]
    }
}

/// A polynomial fit to a dataset.
///
/// `MAX_COEFFS` is the compile-time storage capacity; only the first
/// `degree() + 1` entries of the backing storage are meaningful, as returned
/// by [`Self::coefficients`].
pub struct PolynomialFit<T, const MAX_COEFFS: usize> {
    degree: usize,
    coefficients: [T; MAX_COEFFS],

    /// An indicator of the quality of the fit, ranging from `0.0` to `1.0`;
    /// larger values indicate a better fit.
    ///
    /// This is the fraction of the dataset's variance that is captured by
    /// variance in the fit polynomial, i.e. the coefficient of determination
    /// ("r-squared" in statistics).
    pub confidence: T,
}

impl<T, const MAX_COEFFS: usize> PolynomialFit<T, MAX_COEFFS> {
    /// The degree of the fit polynomial.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// The polynomial coefficients of the fit.
    ///
    /// For each `i`, `coefficients()[i]` is the coefficient of the `i`-th
    /// power of the variable. Has `degree() + 1` elements.
    pub fn coefficients(&self) -> &[T] {
        &self.coefficients[..=self.degree]
    }
}

/// Fits a polynomial to a set of data points using the least-squares method.
///
/// `MAX_SAMPLES` bounds how many data points [`Self::solve`] can fit at
/// once; `x` and `y` may be shorter than that at runtime, but must have the
/// same length.
pub struct LeastSquaresSolver<'a, T, const MAX_SAMPLES: usize> {
    /// The x-coordinate of each data point.
    pub x: &'a [T],
    /// The y-coordinate of each data point.
    pub y: &'a [T],
}

impl<'a, T, const MAX_SAMPLES: usize> LeastSquaresSolver<'a, T, MAX_SAMPLES>
where
    T: Float + Sum + AddAssign + SubAssign + MulAssign + DivAssign,
{
    pub fn new(x: &'a [T], y: &'a [T]) -> Self {
        debug_assert_eq!(x.len(), y.len());
        debug_assert!(x.len() <= MAX_SAMPLES);
        Self { x, y }
    }

    /// Fits a polynomial of the given `degree` to the data points.
    ///
    /// `MAX_COEFFS` bounds the `degree` this can be asked to fit, since it
    /// must be able to store `degree + 1` coefficients.
    ///
    /// Returns `None` when there isn't enough data to fit a curve, or when
    /// the data is degenerate (linearly dependent).
    pub fn solve<const MAX_COEFFS: usize>(
        &self,
        degree: usize,
    ) -> Option<PolynomialFit<T, MAX_COEFFS>> {
        // Shorthand for notation equivalence with the original algorithm:
        // the number of coefficients.
        let n = degree + 1;
        debug_assert!(n <= MAX_COEFFS);

        let m = self.x.len();
        if degree > m {
            // Not enough data to fit a curve.
            return None;
        }

        let tolerance = T::from(PRECISION_ERROR_TOLERANCE).unwrap();

        // Expand the x vector to a matrix A of powers of x: row 0 is all
        // ones, row i is row i - 1 multiplied element-wise by x.
        let mut a = Matrix::<T, MAX_COEFFS, MAX_SAMPLES>::new(m);
        for h in 0..m {
            a.set(0, h, T::one());
            for i in 1..n {
                a.set(i, h, a.get(i - 1, h) * self.x[h]);
            }
        }

        // Apply the Gram-Schmidt process to A to obtain its QR decomposition.

        // Orthonormal basis, one row per basis vector.
        let mut q = Matrix::<T, MAX_COEFFS, MAX_SAMPLES>::new(m);
        // Upper triangular matrix.
        let mut r = Matrix::<T, MAX_COEFFS, MAX_COEFFS>::new(n);
        for j in 0..n {
            for h in 0..m {
                q.set(j, h, a.get(j, h));
            }
            for i in 0..j {
                let d = dot(q.row(j), q.row(i));
                for h in 0..m {
                    q.set(j, h, q.get(j, h) - d * q.get(i, h));
                }
            }

            let row_norm = norm(q.row(j));
            if row_norm < tolerance {
                // Vectors are linearly dependent or zero, so no solution.
                return None;
            }

            let inverse_norm = T::one() / row_norm;
            for h in 0..m {
                q.set(j, h, q.get(j, h) * inverse_norm);
            }
            for i in 0..n {
                r.set(j, i, if i < j { T::zero() } else { dot(q.row(j), a.row(i)) });
            }
        }

        // Solve R B = Qt Y to find B. This is easy because R is upper
        // triangular: work from bottom-right to top-left, computing each
        // coefficient of B in turn.
        let mut coefficients = [T::zero(); MAX_COEFFS];
        for i in (0..n).rev() {
            coefficients[i] = dot(q.row(i), self.y);
            for j in (i + 1..n).rev() {
                coefficients[i] -= r.get(i, j) * coefficients[j];
            }
            coefficients[i] /= r.get(i, i);
        }

        // Calculate the coefficient of determination (confidence) as:
        //   1 - (sum_squared_error / sum_squared_total)
        // where sum_squared_error is the residual sum of squares (variance of
        // the error) and sum_squared_total is the total sum of squares
        // (variance of the data).
        let y_mean = self.y.iter().copied().sum::<T>() / T::from(m).unwrap();

        let mut sum_squared_error = T::zero();
        let mut sum_squared_total = T::zero();
        for h in 0..m {
            let mut term = T::one();
            let mut err = self.y[h] - coefficients[0];
            for i in 1..n {
                term *= self.x[h];
                err -= term * coefficients[i];
            }
            sum_squared_error += err * err;
            let v = self.y[h] - y_mean;
            sum_squared_total += v * v;
        }

        let confidence = if sum_squared_total <= tolerance {
            T::one()
        } else {
            T::one() - (sum_squared_error / sum_squared_total)
        };

        Some(PolynomialFit { degree, coefficients, confidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close<T: Float + core::fmt::Debug>(a: T, b: T) {
        let tolerance = T::from(1e-6).unwrap();
        assert!((a - b).abs() < tolerance, "{a:?} != {b:?}");
    }

    #[test]
    fn not_enough_data() {
        let x = [0.0];
        let y = [0.0];
        let solver = LeastSquaresSolver::<_, 4>::new(&x, &y);
        assert!(solver.solve::<4>(1).is_none());
    }

    #[test]
    fn degenerate_data_is_rejected() {
        // All samples share the same x, so the fit is linearly dependent.
        let x = [1.0, 1.0, 1.0];
        let y = [1.0, 2.0, 3.0];
        let solver = LeastSquaresSolver::<_, 8>::new(&x, &y);
        assert!(solver.solve::<4>(1).is_none());
    }

    #[test]
    fn exact_linear_fit() {
        // y = 2x + 1
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [1.0, 3.0, 5.0, 7.0];
        let solver = LeastSquaresSolver::<_, 8>::new(&x, &y);
        let fit = solver.solve::<4>(1).unwrap();
        assert_close(fit.coefficients()[0], 1.0);
        assert_close(fit.coefficients()[1], 2.0);
        assert_close(fit.confidence, 1.0);
    }

    #[test]
    fn exact_linear_fit_f32() {
        // y = 2x + 1, fit in f32 rather than f64.
        let x: [f32; 4] = [0.0, 1.0, 2.0, 3.0];
        let y: [f32; 4] = [1.0, 3.0, 5.0, 7.0];
        let solver = LeastSquaresSolver::<_, 8>::new(&x, &y);
        let fit = solver.solve::<4>(1).unwrap();
        assert_close(fit.coefficients()[0], 1.0f32);
        assert_close(fit.coefficients()[1], 2.0f32);
        assert_close(fit.confidence, 1.0f32);
    }

    #[test]
    fn exact_quadratic_fit() {
        // y = x^2 - 3x + 2
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: [f64; 5] = core::array::from_fn(|i| {
            let x = i as f64;
            x * x - 3.0 * x + 2.0
        });
        let solver = LeastSquaresSolver::<_, 8>::new(&x, &y);
        let fit = solver.solve::<3>(2).unwrap();
        assert_close(fit.coefficients()[0], 2.0);
        assert_close(fit.coefficients()[1], -3.0);
        assert_close(fit.coefficients()[2], 1.0);
        assert_close(fit.confidence, 1.0);
    }
}
