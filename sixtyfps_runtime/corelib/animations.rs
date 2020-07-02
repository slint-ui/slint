#![warn(missing_docs)]

use std::cell::Cell;

/// The AnimationDriver
pub struct AnimationDriver {
    /// Indicate whether there are any active animations that require a future call to update_animations.
    active_animations: Cell<bool>,
    global_instant: core::pin::Pin<Box<crate::Property<instant::Instant>>>,
}

impl Default for AnimationDriver {
    fn default() -> Self {
        AnimationDriver {
            active_animations: Cell::default(),
            global_instant: Box::pin(crate::Property::new(instant::Instant::now())),
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
        // FIXME! we need to get rid of the contect there
        #[allow(unsafe_code)]
        let dummy_eval_context = crate::EvaluationContext::for_root_component(unsafe {
            core::pin::Pin::new_unchecked(vtable::VRef::from_raw(
                core::ptr::NonNull::dangling(),
                core::ptr::NonNull::dangling(),
            ))
        });
        self.global_instant.as_ref().get(&dummy_eval_context)
    }
}

thread_local!(pub(crate) static CURRENT_ANIMATION_DRIVER : AnimationDriver = AnimationDriver::default());
