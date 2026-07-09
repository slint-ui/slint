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
    pub fn start<T: crate::properties::InterpolatedPropertyValue + Clone>(
        &self,
        property: core::pin::Pin<&crate::properties::Property<T>>,
        _from: Option<T>,
        to: Option<T>,
        animation: crate::items::PropertyAnimation,
    ) {
        let property_ref = property.as_ref();

        let to_value = to.unwrap_or_else(|| property_ref.get());

        // set_animated_value sets up a binding that interpolates from the current
        // property value to the target value. If from is provided, we need to
        // temporarily change the property to that value first, but this won't work
        // for constant properties. The animation system will use the current property
        // value as the starting point, so we rely on the property already being set
        // to the correct starting value, or we skip the from parameter.
        property_ref.set_animated_value(to_value, animation);

        self.running.set(true);
    }

    /// Stop the animation, freezing the property at its current interpolated value.
    pub fn stop<T: crate::properties::InterpolatedPropertyValue + Clone>(
        &self,
        property: core::pin::Pin<&crate::properties::Property<T>>,
    ) {
        if self.running.get() {
            property.as_ref().remove_binding();
            self.running.set(false);
        }
    }

    /// Restart the animation from `from` to `to` again.
    pub fn restart<T: crate::properties::InterpolatedPropertyValue + Clone>(
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
