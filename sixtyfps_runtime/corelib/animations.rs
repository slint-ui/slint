#![warn(missing_docs)]

use std::cell::Cell;

/// The representation of an easing curve, for animations
#[repr(C, u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingCurve {
    /// The linear curve
    Linear,
    /// A Cubic bezier curve, with its 4 parameter
    CubicBezier([f32; 4]),
    //Custom(Box<dyn Fn(f32) -> f32>),
}

impl Default for EasingCurve {
    fn default() -> Self {
        Self::Linear
    }
}

/// The AnimationDriver
pub struct AnimationDriver {
    /// Indicate whether there are any active animations that require a future call to update_animations.
    active_animations: Cell<bool>,
    global_instant: core::pin::Pin<Box<crate::Property<instant::Instant>>>,
    initial_instant: instant::Instant,
}

impl Default for AnimationDriver {
    fn default() -> Self {
        AnimationDriver {
            active_animations: Cell::default(),
            global_instant: Box::pin(crate::Property::new(instant::Instant::now())),
            initial_instant: instant::Instant::now(),
        }
    }
}

impl AnimationDriver {
    /// Iterates through all animations based on the new time tick and updates their state. This should be called by
    /// the windowing system driver for every frame.
    pub fn update_animations(&self, new_tick: instant::Instant) {
        self.active_animations.set(false);
        self.global_instant.as_ref().set(new_tick);
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
    pub fn current_tick(&self) -> instant::Instant {
        self.global_instant.as_ref().get()
    }
}

thread_local!(pub(crate) static CURRENT_ANIMATION_DRIVER : AnimationDriver = AnimationDriver::default());

/// The current instant that is to be used for animation
/// using this function register the current binding as a dependency
pub fn current_tick() -> instant::Instant {
    CURRENT_ANIMATION_DRIVER.with(|driver| driver.current_tick())
}

/// map a value betwen 0 and 1 to another value between 0 and 1 according to the curve
pub fn easing_curve(curve: &EasingCurve, value: f32) -> f32 {
    match curve {
        EasingCurve::Linear => value,
        EasingCurve::CubicBezier([a, b, c, d]) => {
            if !(0.0..=1.0).contains(a) && !(0.0..=1.0).contains(c) {
                return value;
            };
            let curve = lyon::algorithms::geom::cubic_bezier::CubicBezierSegment {
                from: (0., 0.).into(),
                ctrl1: (*a, *b).into(),
                ctrl2: (*c, *d).into(),
                to: (1., 1.).into(),
            };
            let curve = curve.assume_monotonic();
            curve.sample(curve.x(value)).y
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
    test_curve("ease", &EasingCurve::CubicBezier([0.25, 0.1, 0.25, 1.0]));
    test_curve("ease_in", &EasingCurve::CubicBezier([0.42, 0.0, 1.0, 1.0]));
    test_curve("ease_in_out", &EasingCurve::CubicBezier([0.42, 0.0, 0.58, 1.0]));
    test_curve("ease_out", &EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]));
}
*/

/// Update the glibal animation time to the current time
pub(crate) fn update_animations() {
    CURRENT_ANIMATION_DRIVER.with(|driver| {
        match std::env::var("SIXTYFPS_SLOW_ANIMATIONS") {
            Err(_) => driver.update_animations(instant::Instant::now()),
            Ok(val) => {
                let factor = val.parse().unwrap_or(2.);
                driver.update_animations(
                    driver.initial_instant
                        + (instant::Instant::now() - driver.initial_instant).div_f32(factor),
                )
            }
        };
    });
}
