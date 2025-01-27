// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
//! The animation system

use alloc::boxed::Box;
use core::cell::Cell;
#[cfg(not(feature = "std"))]
use num_traits::Float;

mod cubic_bezier {
    //! This is a copy from lyon_algorithms::geom::cubic_bezier implementation
    //! (from lyon_algorithms 0.17)
    type S = f32;
    use euclid::default::Point2D as Point;
    #[allow(unused)]
    use num_traits::Float;
    trait Scalar {
        const ONE: f32 = 1.;
        const THREE: f32 = 3.;
        const HALF: f32 = 0.5;
        const SIX: f32 = 6.;
        const NINE: f32 = 9.;
        fn value(v: f32) -> f32 {
            v
        }
    }
    impl Scalar for f32 {}
    pub struct CubicBezierSegment {
        pub from: Point<S>,
        pub ctrl1: Point<S>,
        pub ctrl2: Point<S>,
        pub to: Point<S>,
    }

    impl CubicBezierSegment {
        /// Sample the x coordinate of the curve at t (expecting t between 0 and 1).
        pub fn x(&self, t: S) -> S {
            let t2 = t * t;
            let t3 = t2 * t;
            let one_t = S::ONE - t;
            let one_t2 = one_t * one_t;
            let one_t3 = one_t2 * one_t;

            self.from.x * one_t3
                + self.ctrl1.x * S::THREE * one_t2 * t
                + self.ctrl2.x * S::THREE * one_t * t2
                + self.to.x * t3
        }

        /// Sample the y coordinate of the curve at t (expecting t between 0 and 1).
        pub fn y(&self, t: S) -> S {
            let t2 = t * t;
            let t3 = t2 * t;
            let one_t = S::ONE - t;
            let one_t2 = one_t * one_t;
            let one_t3 = one_t2 * one_t;

            self.from.y * one_t3
                + self.ctrl1.y * S::THREE * one_t2 * t
                + self.ctrl2.y * S::THREE * one_t * t2
                + self.to.y * t3
        }

        #[inline]
        fn derivative_coefficients(&self, t: S) -> (S, S, S, S) {
            let t2 = t * t;
            (
                -S::THREE * t2 + S::SIX * t - S::THREE,
                S::NINE * t2 - S::value(12.0) * t + S::THREE,
                -S::NINE * t2 + S::SIX * t,
                S::THREE * t2,
            )
        }

        /// Sample the x coordinate of the curve's derivative at t (expecting t between 0 and 1).
        pub fn dx(&self, t: S) -> S {
            let (c0, c1, c2, c3) = self.derivative_coefficients(t);
            self.from.x * c0 + self.ctrl1.x * c1 + self.ctrl2.x * c2 + self.to.x * c3
        }
    }

    impl CubicBezierSegment {
        // This is actually in the Monotonic<CubicBezierSegment<S>> impl
        pub fn solve_t_for_x(&self, x: S, t_range: core::ops::Range<S>, tolerance: S) -> S {
            debug_assert!(t_range.start <= t_range.end);
            let from = self.x(t_range.start);
            let to = self.x(t_range.end);
            if x <= from {
                return t_range.start;
            }
            if x >= to {
                return t_range.end;
            }

            // Newton's method.
            let mut t = x - from / (to - from);
            for _ in 0..8 {
                let x2 = self.x(t);

                if S::abs(x2 - x) <= tolerance {
                    return t;
                }

                let dx = self.dx(t);

                if dx <= S::EPSILON {
                    break;
                }

                t -= (x2 - x) / dx;
            }

            // Fall back to binary search.
            let mut min = t_range.start;
            let mut max = t_range.end;
            let mut t = S::HALF;

            while min < max {
                let x2 = self.x(t);

                if S::abs(x2 - x) < tolerance {
                    return t;
                }

                if x > x2 {
                    min = t;
                } else {
                    max = t;
                }

                t = (max - min) * S::HALF + min;
            }

            t
        }
    }
}

/// The representation of an easing curve, for animations
#[repr(C, u32)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EasingCurve {
    /// The linear curve
    #[default]
    Linear,
    /// A Cubic bezier curve, with its 4 parameters
    CubicBezier([f32; 4]),
    /// Easing curve as defined at: <https://easings.net/#easeInElastic>
    EaseInElastic,
    /// Easing curve as defined at: <https://easings.net/#easeOutElastic>
    EaseOutElastic,
    /// Easing curve as defined at: <https://easings.net/#easeInOutElastic>
    EaseInOutElastic,
    /// Easing curve as defined at: <https://easings.net/#easeInBounce>
    EaseInBounce,
    /// Easing curve as defined at: <https://easings.net/#easeOutBounce>
    EaseOutBounce,
    /// Easing curve as defined at: <https://easings.net/#easeInOutBounce>
    EaseInOutBounce,
    // Custom(Box<dyn Fn(f32) -> f32>),
}

/// Represent an instant, in milliseconds since the AnimationDriver's initial_instant
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Ord, PartialOrd, Eq)]
pub struct Instant(pub u64);

impl core::ops::Sub<Instant> for Instant {
    type Output = core::time::Duration;
    fn sub(self, other: Self) -> core::time::Duration {
        core::time::Duration::from_millis(self.0 - other.0)
    }
}

impl core::ops::Sub<core::time::Duration> for Instant {
    type Output = Instant;
    fn sub(self, other: core::time::Duration) -> Instant {
        Self(self.0 - other.as_millis() as u64)
    }
}

impl core::ops::Add<core::time::Duration> for Instant {
    type Output = Instant;
    fn add(self, other: core::time::Duration) -> Instant {
        Self(self.0 + other.as_millis() as u64)
    }
}

impl core::ops::AddAssign<core::time::Duration> for Instant {
    fn add_assign(&mut self, other: core::time::Duration) {
        self.0 += other.as_millis() as u64;
    }
}

impl core::ops::SubAssign<core::time::Duration> for Instant {
    fn sub_assign(&mut self, other: core::time::Duration) {
        self.0 -= other.as_millis() as u64;
    }
}

impl Instant {
    /// Returns the amount of time elapsed since an other instant.
    ///
    /// Equivalent to `self - earlier`
    pub fn duration_since(self, earlier: Instant) -> core::time::Duration {
        self - earlier
    }

    /// Wrapper around [`std::time::Instant::now()`] that delegates to the backend
    /// and allows working in no_std environments.
    pub fn now() -> Self {
        Self(Self::duration_since_start().as_millis() as u64)
    }

    fn duration_since_start() -> core::time::Duration {
        crate::context::GLOBAL_CONTEXT
            .with(|p| p.get().map(|p| p.platform().duration_since_start()))
            .unwrap_or_default()
    }

    /// Return the number of milliseconds this `Instant` is after the backend has started
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}

/// The AnimationDriver
pub struct AnimationDriver {
    /// Indicate whether there are any active animations that require a future call to update_animations.
    active_animations: Cell<bool>,
    global_instant: core::pin::Pin<Box<crate::Property<Instant>>>,
}

impl Default for AnimationDriver {
    fn default() -> Self {
        AnimationDriver {
            active_animations: Cell::default(),
            global_instant: Box::pin(crate::Property::new_named(
                Instant::default(),
                "i_slint_core::AnimationDriver::global_instant",
            )),
        }
    }
}

impl AnimationDriver {
    /// Iterates through all animations based on the new time tick and updates their state. This should be called by
    /// the windowing system driver for every frame.
    pub fn update_animations(&self, new_tick: Instant) {
        if self.global_instant.as_ref().get_untracked() != new_tick {
            self.active_animations.set(false);
            self.global_instant.as_ref().set(new_tick);
        }
    }

    /// Returns true if there are any active or ready animations. This is used by the windowing system to determine
    /// if a new animation frame is required or not. Returns false otherwise.
    pub fn has_active_animations(&self) -> bool {
        self.active_animations.get()
    }

    /// Tell the driver that there are active animations
    pub fn set_has_active_animations(&self) {
        self.active_animations.set(true);
    }
    /// The current instant that is to be used for animation
    /// using this function register the current binding as a dependency
    pub fn current_tick(&self) -> Instant {
        self.global_instant.as_ref().get()
    }
}

crate::thread_local!(
/// This is the default instance of the animation driver that's used to advance all property animations
/// at the same time.
pub static CURRENT_ANIMATION_DRIVER : AnimationDriver = AnimationDriver::default()
);

/// The current instant that is to be used for animation
/// using this function register the current binding as a dependency
pub fn current_tick() -> Instant {
    CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick())
}

/// Same as [`current_tick`], but also register that one should be running animation
/// on next frame
pub fn animation_tick() -> u64 {
    CURRENT_ANIMATION_DRIVER.with(|driver| {
        driver.set_has_active_animations();
        driver.current_tick().0
    })
}

fn ease_out_bounce_curve(value: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    if value < 1.0 / D1 {
        N1 * value * value
    } else if value < 2.0 / D1 {
        let value = value - (1.5 / D1);
        N1 * value * value + 0.75
    } else if value < 2.5 / D1 {
        let value = value - (2.25 / D1);
        N1 * value * value + 0.9375
    } else {
        let value = value - (2.625 / D1);
        N1 * value * value + 0.984375
    }
}

/// map a value between 0 and 1 to another value between 0 and 1 according to the curve
pub fn easing_curve(curve: &EasingCurve, value: f32) -> f32 {
    match curve {
        EasingCurve::Linear => value,
        EasingCurve::CubicBezier([a, b, c, d]) => {
            if !(0.0..=1.0).contains(a) && !(0.0..=1.0).contains(c) {
                return value;
            };
            let curve = cubic_bezier::CubicBezierSegment {
                from: (0., 0.).into(),
                ctrl1: (*a, *b).into(),
                ctrl2: (*c, *d).into(),
                to: (1., 1.).into(),
            };
            curve.y(curve.solve_t_for_x(value, 0.0..1.0, 0.01))
        }
        EasingCurve::EaseInElastic => {
            const C4: f32 = 2.0 * core::f32::consts::PI / 3.0;

            if value == 0.0 {
                0.0
            } else if value == 1.0 {
                1.0
            } else {
                -f32::powf(2.0, 10.0 * value - 10.0) * f32::sin((value * 10.0 - 10.75) * C4)
            }
        }
        EasingCurve::EaseOutElastic => {
            let c4 = (2.0 * core::f32::consts::PI) / 3.0;

            if value == 0.0 {
                0.0
            } else if value == 1.0 {
                1.0
            } else {
                2.0f32.powf(-10.0 * value) * ((value * 10.0 - 0.75) * c4).sin() + 1.0
            }
        }
        EasingCurve::EaseInOutElastic => {
            const C5: f32 = 2.0 * core::f32::consts::PI / 4.5;

            if value == 0.0 {
                0.0
            } else if value == 1.0 {
                1.0
            } else if value < 0.5 {
                -(f32::powf(2.0, 20.0 * value - 10.0) * f32::sin((20.0 * value - 11.125) * C5))
                    / 2.0
            } else {
                (f32::powf(2.0, -20.0 * value + 10.0) * f32::sin((20.0 * value - 11.125) * C5))
                    / 2.0
                    + 1.0
            }
        }
        EasingCurve::EaseInBounce => 1.0 - ease_out_bounce_curve(1.0 - value),
        EasingCurve::EaseOutBounce => ease_out_bounce_curve(value),
        EasingCurve::EaseInOutBounce => {
            if value < 0.5 {
                (1.0 - ease_out_bounce_curve(1.0 - 2.0 * value)) / 2.0
            } else {
                (1.0 + ease_out_bounce_curve(2.0 * value - 1.0)) / 2.0
            }
        }
    }
}

/*
#[test]
fn easing_test() {
    fn test_curve(name: &str, curve: &EasingCurve) {
        let mut img = image::ImageBuffer::new(500, 500);
        let white = image::Rgba([255 as u8, 255 as u8, 255 as u8, 255 as u8]);

        for x in 0..img.width() {
            let t = (x as f32) / (img.width() as f32);
            let y = easing_curve(curve, t);
            let y = (y * (img.height() as f32)) as u32;
            let y = y.min(img.height() - 1);
            *img.get_pixel_mut(x, img.height() - 1 - y) = white;
        }

        img.save(
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
                .join(format!("{}.png", name)),
        )
        .unwrap();
    }

    test_curve("linear", &EasingCurve::Linear);
    test_curve("linear2", &EasingCurve::CubicBezier([0.0, 0.0, 1.0, 1.0]));
    test_curve("ease", &EasingCurve::CubicBezier([0.25, 0.1, 0.25, 1.0]));
    test_curve("ease_in", &EasingCurve::CubicBezier([0.42, 0.0, 1.0, 1.0]));
    test_curve("ease_in_out", &EasingCurve::CubicBezier([0.42, 0.0, 0.58, 1.0]));
    test_curve("ease_out", &EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]));
}
*/

/// Update the global animation time to the current time
pub fn update_animations() {
    CURRENT_ANIMATION_DRIVER.with(|driver| {
        #[allow(unused_mut)]
        let mut duration = Instant::duration_since_start().as_millis() as u64;
        #[cfg(feature = "std")]
        if let Ok(val) = std::env::var("SLINT_SLOW_ANIMATIONS") {
            let factor = val.parse().unwrap_or(2);
            duration /= factor;
        };
        driver.update_animations(Instant(duration))
    });
}
