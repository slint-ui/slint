#![warn(missing_docs)]

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

/// The AnimationState describes the state reported to the Animated entity when it changes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnimationState {
    /// The animation has started.
    Started,
    /// The animation is in progress.
    Running {
        /// The progress of the running animation in the range between 0 and 1.
        progress: f32,
    },
    /// The animation has stopped.
    Stopped,
}

/// Animated is a trait representing an abstract entity that can be animated over a period of time.
pub trait Animated {
    /// Called by the animation system to notify the animation about the progress, represented by
    /// a value between 0 (start) and 1 (end).
    fn update_animation_state(self: Rc<Self>, state: AnimationState);
    /// Called by the animation system to query the duration the animation is supposed to take.
    fn duration(self: Rc<Self>) -> Duration;
}

#[derive(Clone)]
/// The Animation structure holds everything needed to describe an animation.
struct InternalAnimation {
    /// The current state of the animation.
    state: InternalAnimationState,
    advance_callback: Weak<dyn Animated>,
}

impl InternalAnimation {
    /// Creates a new animation with the specified duration and the advance_callback that'll be called
    /// as time passes.
    pub fn new(advance_callback: Weak<dyn Animated>) -> Self {
        Self {
            state: InternalAnimationState::ReadyToRun { time_elapsed: Duration::default() },
            advance_callback,
        }
    }
}

enum InternalAnimationEntry {
    Allocated(RefCell<InternalAnimation>),
    Free { next_free_idx: Option<usize> },
}

impl InternalAnimationEntry {
    fn as_animation<'a>(&'a self) -> &'a RefCell<InternalAnimation> {
        match self {
            InternalAnimationEntry::Allocated(ref anim) => {
                return anim;
            }
            InternalAnimationEntry::Free { .. } => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
/// The InternalAnimationState describes the three states that an existing animation can be in.
enum InternalAnimationState {
    /// The animation has been scheduled and is ready to transition to running state.
    ReadyToRun {
        /// This is the time that has elapsed since the animation was paused, or 0 if this is
        /// a new animation.
        time_elapsed: Duration,
    },
    /// The animation is currently running.
    Running {
        /// start_time is the tick when the animation was actually started.
        start_time: Instant,
    },
    ReadyToPause {
        /// start_time is the tick when the animation was actually started.
        start_time: Instant,
    },
    /// The animation is paused. It was previously running.
    Paused {
        /// time_elapsed_until_pause is the time between when the animation started and when it was paused.
        /// This is used to calculate the new start_time when resuming.
        time_elapsed_until_pause: Duration,
    },
    /// The animation is stopped. From here must be activately restarted.
    Stopped,
}

/// The AnimationDriver
#[derive(Default)]
pub struct AnimationDriver {
    animations: Vec<InternalAnimationEntry>,
    next_free: Option<usize>,
    len: usize,
    /// Indicate whether there are any active animations that require a future call to update_animations.
    active_animations: Cell<bool>,
}

/// The AnimationHandle can be used to refer to an animation after it's been started, in order to
/// pause or stop it, for example.
#[derive(Copy, Clone, Debug)]
pub struct AnimationHandle(usize);

impl AnimationDriver {
    /// Iterates through all animations based on the new time tick and updates their state. This should be called by
    /// the windowing system driver for every frame.
    pub fn update_animations(&self, new_tick: Instant) {
        self.active_animations.set(false);
        let mut need_new_animation_frame = false;
        let mut i: usize = 0;
        while i < self.animations.len() {
            {
                let animation = match &self.animations[i] {
                    InternalAnimationEntry::Allocated(animation) => animation.borrow().clone(),
                    InternalAnimationEntry::Free { .. } => {
                        i += 1;
                        continue;
                    }
                };
                match animation.state {
                    InternalAnimationState::ReadyToRun { time_elapsed } => {
                        self.set_animation_state(
                            i,
                            InternalAnimationState::Running { start_time: new_tick - time_elapsed },
                        );
                        if time_elapsed == Duration::default() {
                            if let Some(cb) = animation.advance_callback.upgrade() {
                                cb.update_animation_state(AnimationState::Started);
                            }
                        }
                    }
                    _ => {}
                };
            }

            let animation = &self.animations[i].as_animation().borrow().clone();
            match animation.state {
                InternalAnimationState::ReadyToRun { .. } => unreachable!(),
                InternalAnimationState::Running { start_time } => {
                    let time_progress = new_tick.duration_since(start_time).as_millis() as f32;
                    if let Some(cb) = animation.advance_callback.upgrade() {
                        let progress = time_progress / (cb.clone().duration().as_millis() as f32);
                        cb.clone().update_animation_state(AnimationState::Running {
                            progress: progress.min(1.),
                        });
                        if progress >= 1. {
                            cb.update_animation_state(AnimationState::Stopped);

                            self.set_animation_state(i, InternalAnimationState::Stopped);
                        } else {
                            need_new_animation_frame = true;
                        }
                    }
                }
                InternalAnimationState::ReadyToPause { start_time } => {
                    self.set_animation_state(
                        i,
                        InternalAnimationState::Paused {
                            time_elapsed_until_pause: new_tick - start_time,
                        },
                    );
                }
                InternalAnimationState::Paused { .. } => {}
                InternalAnimationState::Stopped => {}
            };
            i += 1;
        }

        self.active_animations.set(self.active_animations.get() | need_new_animation_frame);
    }

    /// Returns true if there are any active or ready animations. This is used by the windowing system to determine
    /// if a new animation frame is required or not. Returns false otherwise.
    pub fn has_active_animations(&self) -> bool {
        self.active_animations.get()
    }

    /// Start a new animation and returns a handle for it.
    pub fn start_animation(&mut self, animation_callback: Weak<dyn Animated>) -> AnimationHandle {
        let animation = InternalAnimation::new(animation_callback);

        let idx = {
            if let Some(free_idx) = self.next_free {
                let entry = &mut self.animations[free_idx];
                if let InternalAnimationEntry::Free { next_free_idx } = entry {
                    self.next_free = *next_free_idx;
                } else {
                    unreachable!();
                }
                *entry = InternalAnimationEntry::Allocated(RefCell::new(animation));
                free_idx
            } else {
                self.animations.push(InternalAnimationEntry::Allocated(RefCell::new(animation)));
                self.animations.len() - 1
            }
        };
        self.active_animations.set(true);
        self.len = self.len + 1;
        AnimationHandle(idx)
    }

    /// Pauses the animation specified by the handle. The animation will not receive any further state updates.
    pub fn pause_animation(&self, handle: AnimationHandle) {
        let animation = self.animations[handle.0].as_animation();
        let state = animation.borrow().state;
        match state {
            InternalAnimationState::ReadyToRun { .. } => {}
            InternalAnimationState::Running { start_time } => {
                animation.borrow_mut().state = InternalAnimationState::ReadyToPause { start_time }
            }
            InternalAnimationState::Paused { .. } => {}
            InternalAnimationState::ReadyToPause { .. } => {}
            InternalAnimationState::Stopped => {}
        }
    }

    /// Resumes the animation specified by the handle. The animation will continue from its last progress.
    pub fn resume_animation(&self, handle: AnimationHandle) {
        let animation = self.animations[handle.0].as_animation();
        let state = animation.borrow().state;
        match state {
            InternalAnimationState::ReadyToRun { .. } => {}
            InternalAnimationState::Running { .. } => {}
            InternalAnimationState::Paused { time_elapsed_until_pause } => {
                animation.borrow_mut().state =
                    InternalAnimationState::ReadyToRun { time_elapsed: time_elapsed_until_pause }
            }
            InternalAnimationState::ReadyToPause { .. } => {}
            InternalAnimationState::Stopped => {}
        }
        self.active_animations.set(true);
    }

    /// Restarts the animation specified by the handle. The progress is set back to zero.
    pub fn restart_animation(&self, handle: AnimationHandle) {
        let animation = self.animations[handle.0].as_animation();
        animation.borrow_mut().state =
            InternalAnimationState::ReadyToRun { time_elapsed: Duration::default() };
        self.active_animations.set(true);
    }

    /// Returns a temporary reference to the animation behind the given handle.
    #[cfg(test)]
    fn get_animation<'a>(&'a self, handle: AnimationHandle) -> &'a RefCell<InternalAnimation> {
        match self.animations[handle.0] {
            InternalAnimationEntry::Allocated(ref anim) => {
                return anim;
            }
            _ => unreachable!(),
        };
    }

    /// Marks the animation specified by the handle as free/unused.
    pub fn free_animation(&mut self, handle: AnimationHandle) {
        self.animations[handle.0] = InternalAnimationEntry::Free { next_free_idx: self.next_free };
        self.next_free = Some(handle.0);
        self.len = self.len - 1;
    }

    fn set_animation_state(&self, index: usize, new_state: InternalAnimationState) {
        let anim = self.animations[index].as_animation();
        anim.borrow_mut().state = new_state;
    }
}

thread_local!(pub(crate) static CURRENT_ANIMATION_DRIVER : Rc<RefCell<AnimationDriver>> = Default::default());

#[cfg(test)]
mod test {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct RecordingAnimation {
        reported_states: Vec<AnimationState>,
        duration: std::time::Duration,
    }

    impl Animated for RefCell<RecordingAnimation> {
        fn update_animation_state(self: Rc<Self>, state: AnimationState) {
            self.borrow_mut().reported_states.push(state)
        }
        fn duration(self: Rc<Self>) -> Duration {
            self.borrow().duration
        }
    }

    #[test]
    fn test_animation_driver() {
        let mut driver = AnimationDriver::default();

        let test_animation = Rc::new(RefCell::new(RecordingAnimation {
            duration: Duration::from_secs(10),
            ..Default::default()
        }));

        assert!(!driver.has_active_animations());

        let handle =
            driver.start_animation(Rc::downgrade(&(test_animation.clone() as Rc<dyn Animated>)));

        assert!(
            matches!(driver.get_animation(handle).borrow().state, InternalAnimationState::ReadyToRun{..})
        );

        assert!(driver.has_active_animations());

        assert_eq!(test_animation.borrow().reported_states.len(), 0);

        let mut test_time = std::time::Instant::now();

        driver.update_animations(test_time);
        assert!(
            matches!(driver.get_animation(handle).borrow().state, InternalAnimationState::Running{..})
        );

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![AnimationState::Started, AnimationState::Running { progress: 0.0f32 }]
        );

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
            ]
        );

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
                AnimationState::Running { progress: 1.0f32 },
                AnimationState::Stopped,
            ]
        );

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
                AnimationState::Running { progress: 1.0f32 },
                AnimationState::Stopped,
            ]
        );

        driver.free_animation(handle);
        assert!(!driver.has_active_animations());
    }

    #[test]
    fn pause_animation() {
        let mut driver = AnimationDriver::default();

        let test_animation = Rc::new(RefCell::new(RecordingAnimation {
            duration: Duration::from_secs(10),
            ..Default::default()
        }));

        let handle =
            driver.start_animation(Rc::downgrade(&(test_animation.clone() as Rc<dyn Animated>)));

        let mut test_time = std::time::Instant::now();

        driver.update_animations(test_time);
        assert!(
            matches!(driver.get_animation(handle).borrow().state, InternalAnimationState::Running{..})
        );

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![AnimationState::Started, AnimationState::Running { progress: 0.0f32 }]
        );

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
            ]
        );

        driver.pause_animation(handle);

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
            ]
        );

        driver.resume_animation(handle);

        test_time += Duration::from_secs(5);
        driver.update_animations(test_time);

        assert_eq!(
            test_animation.borrow().reported_states,
            vec![
                AnimationState::Started,
                AnimationState::Running { progress: 0.0f32 },
                AnimationState::Running { progress: 0.5f32 },
                AnimationState::Running { progress: 1.0f32 },
                AnimationState::Stopped,
            ]
        );

        assert!(!driver.has_active_animations());

        driver.free_animation(handle);
    }

    #[derive(Default)]
    struct SelfPausingAnimation {
        pause_on_next_progress_update: bool,
        driver: Rc<RefCell<AnimationDriver>>,
        handle: Option<AnimationHandle>,
        duration: std::time::Duration,
    }

    impl Animated for RefCell<SelfPausingAnimation> {
        fn update_animation_state(self: Rc<Self>, state: AnimationState) {
            if matches!(state, AnimationState::Running{..}) {
                let this = self.borrow();
                if this.pause_on_next_progress_update {
                    this.driver.borrow().pause_animation(this.handle.unwrap());
                }
            }
        }
        fn duration(self: Rc<Self>) -> Duration {
            self.borrow().duration
        }
    }

    #[test]
    fn test_self_pausing_animation() {
        let driver = Rc::new(RefCell::new(AnimationDriver::default()));

        let anim = Rc::new(RefCell::new(SelfPausingAnimation {
            duration: Duration::from_secs(10),
            ..Default::default()
        }));
        let handle = {
            let a = &mut anim.borrow_mut();
            a.driver = driver.clone();
            a.handle = Some(
                driver
                    .borrow_mut()
                    .start_animation(Rc::downgrade(&(anim.clone() as Rc<dyn Animated>))),
            );
            a.handle.unwrap()
        };

        let mut test_time = std::time::Instant::now();

        driver.borrow().update_animations(test_time);

        assert!(
            matches!(driver.borrow().get_animation(handle).borrow().state, InternalAnimationState::Running{..})
        );

        test_time += Duration::from_secs(1);

        anim.borrow_mut().pause_on_next_progress_update = true;

        test_time += Duration::from_secs(1);
        driver.borrow().update_animations(test_time);

        assert!(
            matches!(driver.borrow().get_animation(handle).borrow().state, InternalAnimationState::ReadyToPause{..})
        );

        test_time += Duration::from_secs(1);
        driver.borrow().update_animations(test_time);

        assert!(
            matches!(driver.borrow().get_animation(handle).borrow().state, InternalAnimationState::Paused{..})
        );
    }
}
