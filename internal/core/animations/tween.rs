// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
    Support for tween animation objects.

    A `TweenAnimation` is a thin handle to a property animation, delegating the
    actual interpolation to `Property::set_animated_value` and `AnimationDriver`,
    which already drive re-evaluation every frame. Unlike timers, a tween is always
    attached to a concrete property and does not need a separate tick registry.
*/

#![warn(missing_docs)]
use core::cell::Cell;
use core::marker::PhantomData;

/// Handle to a running (or stopped) tween animation, driving one property's value
/// from a `from` value to a `to` value over time. Analogous to `crate::timers::Timer`,
/// but delegates the actual interpolation to `Property::set_animated_value` /
/// `AnimationDriver` instead of a separate tick registry, since a tween is always
/// attached to a concrete property.
#[derive(Default)]
pub struct TweenAnimation {
    running: Cell<bool>,
    /// The tween animation cannot be moved between threads
    _phantom: PhantomData<*mut ()>,
}

impl TweenAnimation {
    /// Start (or restart) the animation on `property`, going from `from` to `to`
    /// over `animation.duration`, honoring `animation.easing`.
    ///
    /// TODO(implementer):
    /// - Type conversion: `from`/`to` may be numeric types (f32, i32, etc.) that need to be
    ///   converted to the target type T (e.g., wrapping f32 as LogicalLength)
    /// - Call `property.set(from)` to jump to the start value without animation
    /// - Call `property.set_animated_value(to, animation)` to kick off interpolation;
    ///   this already registers with `AnimationDriver` internally, no extra
    ///   driving-loop wiring needed here.
    /// - Store `running = true`.
    /// - There is currently no public "animation finished" callback from
    ///   `Property::set_animated_value` — decide whether `running()` should reflect
    ///   only "was started and not yet stopped" (simplest) or the underlying
    ///   binding's completion (would need a shared completion flag threaded through
    ///   a wrapper binding).
    pub fn start<T: crate::properties::InterpolatedPropertyValue>(
        &self,
        _property: core::pin::Pin<&crate::properties::Property<T>>,
        _from: Option<T>,
        _to: Option<T>,
        _animation: crate::items::PropertyAnimation,
    ) {
        todo!("if from is Some, set property to from value; call set_animated_value with to (or current value if None); track running state")
    }

    /// Stop the animation, freezing the property at its current interpolated value.
    ///
    /// TODO(implementer): call the equivalent of `Property::remove_binding` (see
    /// `internal/core/properties/properties_animations.rs:299`, which itself calls
    /// `set_animated_value(get(), PropertyAnimation::default())`) so the animated
    /// binding is removed without visibly snapping the value. Set `running = false`.
    pub fn stop<T: crate::properties::InterpolatedPropertyValue>(
        &self,
        _property: core::pin::Pin<&crate::properties::Property<T>>,
    ) {
        todo!("remove the animated binding on `property`, freezing its current value")
    }

    /// Restart the animation from `from` to `to` again.
    ///
    /// TODO(implementer): equivalent to calling `start` again with the same
    /// parameters; codegen re-passes `from`/`to`/`animation` each time (see
    /// `update_animations()` in generated code), so this may just delegate to `start`.
    pub fn restart<T: crate::properties::InterpolatedPropertyValue>(
        &self,
        property: core::pin::Pin<&crate::properties::Property<T>>,
        from: Option<T>,
        to: Option<T>,
        animation: crate::items::PropertyAnimation,
    ) {
        self.start(property, from, to, animation);
    }

    /// Returns true if the animation was started and not yet stopped.
    pub fn running(&self) -> bool {
        self.running.get()
    }
}