// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore singleshot

/*!
    Support for timers.

    Timers are just a bunch of callbacks sorted by expiry date.
*/

#![warn(missing_docs)]
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};

use crate::animations::Instant;

type TimerCallback = Box<dyn FnMut()>;
type SingleShotTimerCallback = Box<dyn FnOnce()>;

/// The TimerMode specifies what should happen after the timer fired.
///
/// Used by the [`Timer::start()`] function.
#[derive(Copy, Clone)]
#[repr(u8)]
#[non_exhaustive]
pub enum TimerMode {
    /// A SingleShot timer is fired only once.
    SingleShot,
    /// A Repeated timer is fired repeatedly until it is stopped or dropped.
    Repeated,
}

/// Timer is a handle to the timer system that triggers a callback after a specified
/// period of time.
///
/// Use [`Timer::start()`] to create a timer that repeatedly triggers a callback, or
/// [`Timer::single_shot`] to trigger a callback only once.
///
/// The timer will automatically stop when dropped. You must keep the Timer object
/// around for as long as you want the timer to keep firing.
///
/// Timers can only be used in the thread that runs the Slint event loop. They don't
/// fire if used in another thread.
///
/// ## Example
/// ```rust,no_run
/// # i_slint_backend_testing::init_no_event_loop();
/// use slint::{Timer, TimerMode};
/// let timer = Timer::default();
/// timer.start(TimerMode::Repeated, std::time::Duration::from_millis(200), move || {
///    println!("This will be printed every 200ms.");
/// });
/// // ... more initialization ...
/// slint::run_event_loop();
/// ```
#[derive(Default)]
pub struct Timer {
    /// Identifies both the list this timer belongs to and its slot in that list, packed
    /// into one word. See [`Timer::resolve`] for the encoding.
    id: Cell<Option<NonZeroUsize>>,
    /// The timer cannot be moved between treads
    _phantom: core::marker::PhantomData<*mut ()>,
}

/// `Timer` must stay exactly one pointer wide: the C++ `slint::Timer` mirrors it by hand as
/// a single `uintptr_t` (see `api/cpp/include/private/slint_timer.h`), and native items such
/// as `TooltipArea` and `ContextMenu` embed it in structs that C++ lays out. Widening it
/// silently corrupts those items rather than failing to build, so pin the size here.
const _: () = assert!(
    core::mem::size_of::<Timer>() == core::mem::size_of::<usize>(),
    "Timer must stay one word wide to match the hand-written C++ slint::Timer"
);

impl Timer {
    /// Starts the timer with the given mode and interval, in order for the callback to called when the
    /// timer fires. If the timer has been started previously, then it will be restarted, no matter if
    /// it has already been fired or not.
    ///
    /// Arguments:
    /// * `mode`: The timer mode to apply, i.e. whether to repeatedly fire the timer or just once.
    /// * `interval`: The duration from now until when the timer should fire the first time, and subsequently
    ///   for repeated [`Repeated`](TimerMode::Repeated) timers.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    pub fn start(
        &self,
        mode: TimerMode,
        interval: core::time::Duration,
        callback: impl FnMut() + 'static,
    ) {
        // Keep firing on the list this timer already belongs to. Only a timer that was
        // never started, or whose list went away with its context, picks a new one.
        let (timers, slot) = match self.resolve() {
            Some(resolved) => resolved,
            None => {
                let Some(timers) = current_timers() else { return };
                (timers, None)
            }
        };

        let handle = list_handle(&timers);
        let slot = timers.borrow_mut().start_or_restart_timer(
            slot,
            mode,
            interval,
            CallbackVariant::MultiFire(Box::new(callback)),
        );
        self.set_ids(handle, Some(slot));
    }

    /// Starts the timer on `ctx` rather than on whichever context happens to be current.
    ///
    /// A timer that already belongs to a live context keeps firing on that one; this only
    /// decides where a timer that has never been started registers. Used by items, which
    /// own their `Timer` as a field but can reach their window's context when they start it.
    pub(crate) fn start_on(
        &self,
        ctx: &crate::SlintContext,
        mode: TimerMode,
        interval: core::time::Duration,
        callback: impl FnMut() + 'static,
    ) {
        if self.resolve().is_none() {
            self.set_ids(list_handle(&ctx.0.timers), None);
        }
        self.start(mode, interval, callback);
    }

    /// Starts the timer with the duration and the callback to called when the
    /// timer fires. It is fired only once and then deleted.
    ///
    /// Arguments:
    /// * `duration`: The duration from now until when the timer should fire.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    ///
    /// ## Example
    /// ```rust
    /// # i_slint_backend_testing::init_no_event_loop();
    /// use slint::Timer;
    /// Timer::single_shot(std::time::Duration::from_millis(200), move || {
    ///    println!("This will be printed after 200ms.");
    /// });
    /// ```
    pub fn single_shot(duration: core::time::Duration, callback: impl FnOnce() + 'static) {
        if let Some(timers) = current_timers() {
            timers.borrow_mut().start_or_restart_timer(
                None,
                TimerMode::SingleShot,
                duration,
                CallbackVariant::SingleShot(Box::new(callback)),
            );
        }
    }

    /// Stops the previously started timer. Does nothing if the timer has never been started.
    pub fn stop(&self) {
        if let Some((timers, slot)) = self.started() {
            timers.borrow_mut().deactivate_timer(slot);
        }
    }

    /// Restarts the timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    pub fn restart(&self) {
        if let Some((timers, slot)) = self.started() {
            timers.borrow_mut().deactivate_timer(slot);
            timers.borrow_mut().activate_timer(slot);
        }
    }

    /// Returns true if the timer is running; false otherwise.
    pub fn running(&self) -> bool {
        self.started().is_some_and(|(timers, slot)| timers.borrow().timers[slot].running)
    }

    /// Change the duration of timer. If the timer was is running (see [`Self::running()`]),
    /// then the time when the callback will be next invoked is re-calculated to be in the
    /// specified duration relative to when this function is called.
    ///
    /// Arguments:
    /// * `interval`: The duration from now until when the timer should fire. And the period of that timer
    ///   for [`Repeated`](TimerMode::Repeated) timers.
    pub fn set_interval(&self, interval: core::time::Duration) {
        if let Some((timers, slot)) = self.started() {
            timers.borrow_mut().set_interval(slot, interval);
        }
    }

    /// Returns the interval of the timer. If the timer was never started, the returned duration is 0ms.
    pub fn interval(&self) -> core::time::Duration {
        self.started()
            .map(|(timers, slot)| timers.borrow().timers[slot].duration)
            .unwrap_or_default()
    }

    /// A timer that will register in `timers` when started, rather than in whichever list
    /// is current at that point. Backs [`SlintContext::new_timer`](crate::SlintContext::new_timer).
    pub(crate) fn with_list(timers: &TimerListRc) -> Self {
        let timer = Self::default();
        timer.set_ids(list_handle(timers), None);
        timer
    }

    /// Decodes `id` into the list this timer belongs to and its slot in that list.
    ///
    /// `None` when the timer has no list at all, or when that list has gone away with the
    /// context owning it — in which case the slot it names no longer means anything, and
    /// every operation but [`Self::start`] does nothing. The inner `Option` is `None` for a
    /// timer that has a list but was never started, as [`Self::with_list`] produces.
    fn resolve(&self) -> Option<(TimerListRc, Option<usize>)> {
        let id = usize::from(self.id.get()?);
        let timers = list_from_handle(id >> LIST_SHIFT)?;
        Some((timers, (id & SLOT_MASK).checked_sub(1)))
    }

    /// The list and slot of a timer that has been started and whose list is still alive.
    /// `None` in every other case, which is what all operations but [`Self::start`] want.
    fn started(&self) -> Option<(TimerListRc, usize)> {
        let (timers, slot) = self.resolve()?;
        Some((timers, slot?))
    }

    fn set_ids(&self, list: usize, slot: Option<usize>) {
        let slot = slot.map_or(0, |slot| slot + 1);
        debug_assert!(slot <= SLOT_MASK, "too many timers in one list");
        debug_assert!(list <= usize::MAX >> LIST_SHIFT, "too many timer lists");
        self.id.set(NonZeroUsize::new((list << LIST_SHIFT) | slot));
    }
}

/// Registers a single-shot timer in `timers` rather than in whichever list is current.
/// Backs [`SlintContext::single_shot`](crate::SlintContext::single_shot).
pub(crate) fn single_shot_on(
    timers: &TimerListRc,
    duration: core::time::Duration,
    callback: impl FnOnce() + 'static,
) {
    timers.borrow_mut().start_or_restart_timer(
        None,
        TimerMode::SingleShot,
        duration,
        CallbackVariant::SingleShot(Box::new(callback)),
    );
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((timers, slot)) = self.started() {
            #[cfg(target_os = "android")]
            if timers.borrow().timers.is_empty() {
                // There seems to be a bug in android thread_local where try_with recreates the already thread local.
                // But we are called from the drop of another thread local, just ignore the drop then
                return;
            }
            let callback = timers.borrow_mut().remove_timer(slot);
            // drop the callback without having the list borrowed
            drop(callback);
        }
    }
}

enum CallbackVariant {
    Empty,
    MultiFire(TimerCallback),
    SingleShot(SingleShotTimerCallback),
}

struct TimerData {
    duration: core::time::Duration,
    mode: TimerMode,
    running: bool,
    /// Set to true when it is removed when the callback is still running
    removed: bool,
    /// true if it is in the cached the active_timers list in the maybe_activate_timers stack
    being_activated: bool,

    callback: CallbackVariant,
}

#[derive(Clone, Copy)]
struct ActiveTimer {
    id: usize,
    timeout: Instant,
}

/// TimerList provides the interface to the event loop for activating times and
/// determining the nearest timeout.
#[derive(Default)]
pub struct TimerList {
    timers: slab::Slab<TimerData>,
    active_timers: Vec<ActiveTimer>,
    /// If a callback is currently running, this is the id of the currently running callback
    callback_active: Option<usize>,
    /// This list's index in [`ThreadTimers::lists`], assigned the first time a timer id naming it
    /// is handed out. Cached here so encoding an id doesn't have to search the registry.
    handle: Option<usize>,
    /// The context this list belongs to, so that a deadline is computed on the same clock it
    /// is later compared against. `None` until a context takes the list over — the origin is
    /// the platform's start time, and without a platform there is no origin.
    context: Option<crate::SlintContextWeak>,
}

impl TimerList {
    /// Returns the timeout of the timer that should fire the soonest, or None if there
    /// is no timer active.
    pub fn next_timeout() -> Option<Instant> {
        current_timers().and_then(|timers| timers.borrow().first_timeout())
    }

    /// Activates any expired timers by calling their callback function. Returns true if any timers were
    /// activated; false otherwise.
    pub fn maybe_activate_timers(now: Instant) -> bool {
        current_timers().is_some_and(|timers| Self::activate_expired(&timers, now))
    }

    /// The current instant on this list's clock, or the zero origin while no context owns
    /// it — the same origin the deadlines of any timer started that early were computed
    /// against, so they stay consistent until a context adopts them.
    fn now(&self) -> Instant {
        self.context
            .as_ref()
            .and_then(|ctx| ctx.upgrade())
            .map_or_else(Instant::default, |ctx| Instant::now(&ctx))
    }

    /// The timeout of the timer in this list that should fire the soonest, or None if
    /// none is active.
    pub(crate) fn first_timeout(&self) -> Option<Instant> {
        self.active_timers.first().map(|first_active_timer| first_active_timer.timeout)
    }

    /// Activates the timers in `timers` that have expired by `now`. Returns true if any
    /// timer was activated; false otherwise.
    ///
    /// Takes the list by `&RefCell` rather than `&mut self` because the borrow has to be
    /// released around every callback: a callback may start, stop or drop timers.
    pub(crate) fn activate_expired(timers: &RefCell<Self>, now: Instant) -> bool {
        // Shortcut: Is there any timer worth activating?
        if timers.borrow().first_timeout().map(|timeout| now < timeout).unwrap_or(false) {
            return false;
        }

        assert!(timers.borrow().callback_active.is_none(), "Recursion in timer code");

        // Re-register all timers that expired but are repeating, as well as all that haven't expired yet. This is
        // done in one shot to ensure a consistent state by the time the callbacks are invoked.
        let expired_timers = {
            let mut timers = timers.borrow_mut();

            // Empty active_timers and rebuild it, to preserve insertion order across expired and not expired timers.
            let mut active_timers = core::mem::take(&mut timers.active_timers);

            let expired_vs_remaining_timers_partition_point =
                active_timers.partition_point(|active_timer| active_timer.timeout <= now);

            let (expired_timers, timers_not_activated_this_time) =
                active_timers.split_at(expired_vs_remaining_timers_partition_point);

            for expired_timer in expired_timers {
                let timer = &mut timers.timers[expired_timer.id];
                assert!(!timer.being_activated);
                timer.being_activated = true;

                if matches!(timers.timers[expired_timer.id].mode, TimerMode::Repeated) {
                    timers.activate_timer(expired_timer.id);
                } else {
                    timers.timers[expired_timer.id].running = false;
                }
            }

            for future_timer in timers_not_activated_this_time.iter() {
                timers.register_active_timer(*future_timer);
            }

            // turn `expired_timers` slice into a truncated vec.
            active_timers.truncate(expired_vs_remaining_timers_partition_point);
            active_timers
        };

        let any_activated = !expired_timers.is_empty();

        for active_timer in expired_timers.into_iter() {
            let mut callback = {
                let mut timers = timers.borrow_mut();

                timers.callback_active = Some(active_timer.id);

                // have to release the borrow on `timers` before invoking the callback,
                // so here we temporarily move the callback out of its permanent place
                core::mem::replace(
                    &mut timers.timers[active_timer.id].callback,
                    CallbackVariant::Empty,
                )
            };

            match callback {
                CallbackVariant::Empty => (),
                CallbackVariant::MultiFire(ref mut cb) => cb(),
                CallbackVariant::SingleShot(cb) => {
                    cb();
                    timers.borrow_mut().callback_active = None;
                    timers.borrow_mut().timers.remove(active_timer.id);
                    continue;
                }
            };

            let mut timers = timers.borrow_mut();

            let callback_register = &mut timers.timers[active_timer.id].callback;

            // only emplace back the callback if its permanent store is still Empty:
            // if not, it means the invoked callback has restarted its own timer with a new callback
            if matches!(callback_register, CallbackVariant::Empty) {
                *callback_register = callback;
            }

            timers.callback_active = None;
            let t = &mut timers.timers[active_timer.id];
            if t.removed {
                timers.timers.remove(active_timer.id);
            } else {
                t.being_activated = false;
            }
        }
        any_activated
    }

    fn start_or_restart_timer(
        &mut self,
        id: Option<usize>,
        mode: TimerMode,
        duration: core::time::Duration,
        callback: CallbackVariant,
    ) -> usize {
        let mut timer_data = TimerData {
            duration,
            mode,
            running: false,
            removed: false,
            callback,
            being_activated: false,
        };
        let inactive_timer_id = if let Some(id) = id {
            self.deactivate_timer(id);
            timer_data.being_activated = self.timers[id].being_activated;
            self.timers[id] = timer_data;
            id
        } else {
            self.timers.insert(timer_data)
        };
        self.activate_timer(inactive_timer_id);
        inactive_timer_id
    }

    fn deactivate_timer(&mut self, id: usize) {
        let mut i = 0;
        while i < self.active_timers.len() {
            if self.active_timers[i].id == id {
                self.active_timers.remove(i);
                self.timers[id].running = false;
                debug_assert!(!self.active_timers.iter().any(|t| t.id == id));
                break;
            } else {
                i += 1;
            }
        }
    }

    fn activate_timer(&mut self, id: usize) {
        self.register_active_timer(ActiveTimer {
            id,
            timeout: self.now() + self.timers[id].duration,
        });
    }

    fn register_active_timer(&mut self, new_active_timer: ActiveTimer) {
        debug_assert!(!self.active_timers.iter().any(|t| t.id == new_active_timer.id));
        let insertion_index = self
            .active_timers
            .partition_point(|existing_timer| existing_timer.timeout < new_active_timer.timeout);
        self.active_timers.insert(insertion_index, new_active_timer);
        self.timers[new_active_timer.id].running = true;
    }

    fn remove_timer(&mut self, id: usize) -> CallbackVariant {
        self.deactivate_timer(id);
        let t = &mut self.timers[id];
        if t.being_activated {
            t.removed = true;
            CallbackVariant::Empty
        } else {
            self.timers.remove(id).callback
        }
    }

    fn set_interval(&mut self, id: usize, duration: core::time::Duration) {
        let timer = &self.timers[id];
        if timer.running {
            self.deactivate_timer(id);
            self.timers[id].duration = duration;
            self.activate_timer(id);
        } else {
            self.timers[id].duration = duration;
        }
    }
}

/// A timer list, shared so that [`Timer`] handles can point at the one they registered in
/// without needing to know who owns it.
pub(crate) type TimerListRc = alloc::rc::Rc<RefCell<TimerList>>;

/// How a [`Timer`]'s `id` splits into the list it belongs to and its slot in that list.
///
/// The high part is the list's index in [`ThreadTimers::lists`] plus one, so the whole word
/// is never zero; the low part is the slot plus one, or zero for a timer that has a list but
/// hasn't been started. Packing both into the existing word is what keeps `Timer` one
/// pointer wide, which the C++ ABI requires — see the assertion next to the struct.
const LIST_SHIFT: u32 = if usize::BITS >= 64 { 32 } else { 20 };
const SLOT_MASK: usize = (1 << LIST_SHIFT) - 1;

/// This thread's timer bookkeeping: which list a handle names, and who owns the list that
/// no context has taken over yet.
#[derive(Default)]
struct ThreadTimers {
    /// The lists this thread has handed out ids for, weakly so that a context's list dies
    /// with it. Slots are never reused: a stale id must resolve to nothing rather than to
    /// some later list that happens to sit at the same index. Dead entries are emptied in
    /// [`list_handle`] so they don't pin the allocation of a list that is already gone.
    lists: Vec<alloc::rc::Weak<RefCell<TimerList>>>,
    /// Owns the list that timers register in before this thread has a context, until the
    /// first context adopts it. Nothing else would keep it alive: `lists` is weak, and a
    /// `Timer` holds an id rather than a reference.
    pending: Option<TimerListRc>,
}

crate::thread_local!(static TIMERS : RefCell<ThreadTimers> = RefCell::default());

/// The handle identifying `timers` in an id, registering it on first use. Handles are the
/// registry index plus one, so that the high part of an id is never zero.
fn list_handle(timers: &TimerListRc) -> usize {
    *timers.borrow_mut().handle.get_or_insert_with(|| {
        TIMERS.with(|thread_timers| {
            let lists = &mut thread_timers.borrow_mut().lists;
            // A `Weak` to a list whose context is gone keeps that list's allocation alive
            // even though its contents were dropped with the context. Registering happens
            // once per context, so take the opportunity to release those. The slots stay
            // occupied: reusing an index would let a stale id name a different list.
            for entry in lists.iter_mut() {
                if entry.strong_count() == 0 {
                    *entry = alloc::rc::Weak::new();
                }
            }
            lists.push(alloc::rc::Rc::downgrade(timers));
            lists.len()
        })
    })
}

/// The list a handle refers to, or `None` once it has been dropped.
fn list_from_handle(handle: usize) -> Option<TimerListRc> {
    TIMERS
        .try_with(|thread_timers| {
            thread_timers.borrow().lists.get(handle - 1).and_then(alloc::rc::Weak::upgrade)
        })
        .ok()
        .flatten()
}

/// The list that timers started on this thread register in: this thread's context's list,
/// or the pending one when there is no context yet.
///
/// Returns `None` only when thread-local storage is already being torn down, hence the
/// `try_with`: `Timer::drop` runs during thread exit.
fn current_timers() -> Option<TimerListRc> {
    let on_context = crate::context::GLOBAL_CONTEXT
        .try_with(|ctx| ctx.get().map(|ctx| ctx.0.timers.clone()))
        .ok()?;
    if on_context.is_some() {
        return on_context;
    }
    TIMERS
        .try_with(|thread_timers| {
            thread_timers.borrow_mut().pending.get_or_insert_with(Default::default).clone()
        })
        .ok()
}

/// Hands the pending list over to a context being constructed, so that timers started
/// before this thread had one keep working — they hold a `Weak` to this very list.
///
/// The next context to be created starts from an empty list.
/// Points `timers` at the context that now owns it, so its deadlines are measured on that
/// context's clock.
pub(crate) fn set_owning_context(timers: &TimerListRc, ctx: &crate::SlintContext) {
    timers.borrow_mut().context = Some(ctx.downgrade());
}

pub(crate) fn take_pending_timers() -> TimerListRc {
    TIMERS
        .try_with(|thread_timers| thread_timers.borrow_mut().pending.take())
        .ok()
        .flatten()
        .unwrap_or_default()
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use core::ffi::c_void;

    /// A handle to the timer `id`, which names both its list and its slot. An id of 0 means
    /// "no timer yet", so C++ can pass one straight back to `slint_timer_start`.
    fn timer_from_id(id: usize) -> Timer {
        let timer = Timer::default();
        timer.id.set(NonZeroUsize::new(id));
        timer
    }

    /// Runs `f` on the timer `id` names, without unregistering it when the borrowed handle
    /// goes away: C++ owns the timer and calls `slint_timer_destroy` in its destructor.
    fn with_timer<R>(id: usize, f: impl FnOnce(&Timer) -> R) -> R {
        let timer = timer_from_id(id);
        let result = f(&timer);
        timer.id.take();
        result
    }

    struct WrapFn {
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for WrapFn {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl WrapFn {
        fn call(&self) {
            (self.callback)(self.user_data)
        }
    }

    /// Start a timer with the given mode, duration in millisecond and callback. A timer id may be provided (first argument).
    /// A value of -1 for the timer id means a new timer is to be allocated.
    /// The (new) timer id is returned.
    /// The timer MUST be destroyed with slint_timer_destroy.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_start(
        id: usize,
        mode: TimerMode,
        duration: u64,
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        let wrap = WrapFn { callback, user_data, drop_user_data };
        let timer = timer_from_id(id);
        if duration > i64::MAX as u64 {
            // negative duration? stop the timer
            timer.stop();
        } else {
            timer.start(mode, core::time::Duration::from_millis(duration), move || wrap.call());
        }
        timer.id.take().map(usize::from).unwrap_or(0)
    }

    /// Execute a callback with a delay in millisecond
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_singleshot(
        delay: u64,
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let wrap = WrapFn { callback, user_data, drop_user_data };
        Timer::single_shot(core::time::Duration::from_millis(delay), move || wrap.call());
    }

    /// Stop a timer and free its raw data
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_destroy(id: usize) {
        drop(timer_from_id(id));
    }

    /// Stop a timer
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_stop(id: usize) {
        with_timer(id, Timer::stop)
    }

    /// Restart a repeated timer
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_restart(id: usize) {
        with_timer(id, Timer::restart)
    }

    /// Returns true if the timer is running; false otherwise.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_running(id: usize) -> bool {
        with_timer(id, Timer::running)
    }

    /// Returns the interval in milliseconds. 0 when the timer was never started.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_timer_interval(id: usize) -> u64 {
        with_timer(id, |timer| timer.interval().as_millis() as u64)
    }
}

/**
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    timer_200: Timer,
    timer_200_called: usize,
    timer_500: Timer,
    timer_500_called: usize,
    timer_once: Timer,
    timer_once_called: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
// Note: state will be leaked because of circular dependencies: don't do that in production
let state_ = state.clone();
state.borrow_mut().timer_200.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().timer_200_called += 1;
});
let state_ = state.clone();
state.borrow_mut().timer_once.start(TimerMode::Repeated, Duration::from_millis(300), move || {
    state_.borrow_mut().timer_once_called += 1;
    state_.borrow().timer_once.stop();
});
let state_ = state.clone();
state.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
    state_.borrow_mut().timer_500_called += 1;
});
slint::platform::update_timers_and_animations();
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 0);
assert_eq!(state.borrow().timer_once_called, 0);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 1);
assert_eq!(state.borrow().timer_once_called, 0);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 1);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_backend_testing::mock_elapsed_time(200); // total: 500
assert_eq!(state.borrow().timer_200_called, 2);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 1);
for _ in 0..10 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
// total: 1500
assert_eq!(state.borrow().timer_200_called, 7);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
state.borrow().timer_once.restart();
state.borrow().timer_200.restart();
state.borrow().timer_500.stop();
slint::platform::update_timers_and_animations();
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 7);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 8);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 8);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_backend_testing::mock_elapsed_time(1000);
slint::platform::update_timers_and_animations();
slint::platform::update_timers_and_animations();
// Despite 1000ms have passed, the 200 timer is only called once because we didn't call update_timers_and_animations in between
assert_eq!(state.borrow().timer_200_called, 9);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
let state_ = state.clone();
state.borrow().timer_200.start(TimerMode::SingleShot, Duration::from_millis(200), move || {
    state_.borrow_mut().timer_200_called += 1;
});
for _ in 0..5 {
    i_slint_backend_testing::mock_elapsed_time(75);
}
assert_eq!(state.borrow().timer_200_called, 10);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
state.borrow().timer_200.restart();
for _ in 0..5 {
    i_slint_backend_testing::mock_elapsed_time(75);
}
assert_eq!(state.borrow().timer_200_called, 11);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);

// Test re-starting from a callback
let state_ = state.clone();
state.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
    state_.borrow_mut().timer_500_called += 1;
    let state__ = state_.clone();
    state_.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
        state__.borrow_mut().timer_500_called += 1000;
    });
    let state__ = state_.clone();
    state_.borrow_mut().timer_200.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        state__.borrow_mut().timer_200_called += 1000;
    });
});
for _ in 0..20 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_200_called, 7011);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3004);

// Test set interval
let state_ = state.clone();
state.borrow_mut().timer_200.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().timer_200_called += 1;
});
let state_ = state.clone();
state.borrow_mut().timer_once.start(TimerMode::Repeated, Duration::from_millis(300), move || {
    state_.borrow_mut().timer_once_called += 1;
    state_.borrow().timer_once.stop();
});
let state_ = state.clone();
state.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
    state_.borrow_mut().timer_500_called += 1;
});

let state_ = state.clone();
slint::platform::update_timers_and_animations();
for _ in 0..5 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
slint::platform::update_timers_and_animations();
assert_eq!(state.borrow().timer_200_called, 7013);
assert_eq!(state.borrow().timer_once_called, 3);
assert_eq!(state.borrow().timer_500_called, 3005);

for _ in 0..20 {
    state.borrow().timer_200.set_interval(Duration::from_millis(200 * 2));
    state.borrow().timer_once.set_interval(Duration::from_millis(300 * 2));
    state.borrow().timer_500.set_interval(Duration::from_millis(500 * 2));

    assert_eq!(state.borrow().timer_200_called, 7013);
    assert_eq!(state.borrow().timer_once_called, 3);
    assert_eq!(state.borrow().timer_500_called, 3005);

    i_slint_backend_testing::mock_elapsed_time(100);
}

slint::platform::update_timers_and_animations();
for _ in 0..9 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
slint::platform::update_timers_and_animations();
assert_eq!(state.borrow().timer_200_called, 7015);
assert_eq!(state.borrow().timer_once_called, 3);
assert_eq!(state.borrow().timer_500_called, 3006);

state.borrow().timer_200.stop();
state.borrow().timer_500.stop();

state.borrow_mut().timer_once.restart();
for _ in 0..4 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_once_called, 3);
for _ in 0..4 {
    i_slint_backend_testing::mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_once_called, 4);

state.borrow_mut().timer_once.stop();
i_slint_backend_testing::mock_elapsed_time(1000);

assert_eq!(state.borrow().timer_200_called, 7015);
assert_eq!(state.borrow().timer_once_called, 4);
assert_eq!(state.borrow().timer_500_called, 3006);
```
 */
#[cfg(doctest)]
const _TIMER_TESTS: () = ();

/**
 * Test that deleting an active timer from a timer event works.
```rust
// There is a 200 ms timer that increase variable1
// after 500ms, that timer is destroyed by a single shot timer,
// and a new new timer  increase variable2
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    repeated_timer: Timer,
    variable1: usize,
    variable2: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
// Note: state will be leaked because of circular dependencies: don't do that in production
let state_ = state.clone();
state.borrow_mut().repeated_timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().variable1 += 1;
});
let state_ = state.clone();
Timer::single_shot(Duration::from_millis(500), move || {
    state_.borrow_mut().repeated_timer = Default::default();
    let state = state_.clone();
    state_.borrow_mut().repeated_timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        state.borrow_mut().variable2 += 1;
    })
} );
i_slint_backend_testing::mock_elapsed_time(10);
assert_eq!(state.borrow().variable1, 0);
assert_eq!(state.borrow().variable2, 0);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1);
assert_eq!(state.borrow().variable2, 0);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_backend_testing::mock_elapsed_time(100);
// More than 500ms have elapsed, the single shot timer should have been activated, but that has no effect on variable 1 and 2
// This should just restart the timer so that the next change should happen 200ms from now
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_backend_testing::mock_elapsed_time(110);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 1);
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 1);
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 2);
```
 */
#[cfg(doctest)]
const _BUG3019: () = ();

/**
 * Test that starting a singleshot timer works
```rust
// There is a 200 ms singleshot timer that increase variable1
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    variable1: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
let state_ = state.clone();
let timer = Timer::default();

timer.start(TimerMode::SingleShot, Duration::from_millis(200), move || {
    state_.borrow_mut().variable1 += 1;
});

// Singleshot timer set up and run...
assert!(timer.running());
i_slint_backend_testing::mock_elapsed_time(10);
assert!(timer.running());
assert_eq!(state.borrow().variable1, 0);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Restart a previously set up singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 1);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Stop a non-running singleshot timer
timer.stop();
assert!(!timer.running());
assert_eq!(state.borrow().variable1, 2);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Stop a running singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 2);
i_slint_backend_testing::mock_elapsed_time(10);
timer.stop();
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// set_interval on a non-running singleshot timer
timer.set_interval(Duration::from_millis(300));
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(1000);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
timer.restart();
assert!(timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 3);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(300);
assert_eq!(state.borrow().variable1, 3); // It's singleshot, it only triggers once!
assert!(!timer.running());

// set_interval on a running singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 3);
i_slint_backend_testing::mock_elapsed_time(290);
timer.set_interval(Duration::from_millis(400));
assert!(timer.running());
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 3);
assert!(timer.running());
i_slint_backend_testing::mock_elapsed_time(250);
assert_eq!(state.borrow().variable1, 4);
assert!(!timer.running());
i_slint_backend_testing::mock_elapsed_time(400);
assert_eq!(state.borrow().variable1, 4); // It's singleshot, it only triggers once!
assert!(!timer.running());
```
 */
#[cfg(doctest)]
const _SINGLESHOT_START: () = ();

/**
 * Test that it's possible to start a new timer from within Drop of a timer's closure.
 * This may happen when a timer's closure is dropped, that closure holds the last reference
 * to a component, that component is destroyed, and the accesskit code schedules a reload_tree
 * via a single shot.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::Cell, time::Duration};
#[derive(Default)]
struct CapturedInClosure {
    last_fired: Option<Rc<Cell<bool>>>,
}
impl Drop for CapturedInClosure {
    fn drop(&mut self) {
        if let Some(last_fired) = self.last_fired.as_ref().cloned() {
            Timer::single_shot(Duration::from_millis(100), move || last_fired.set(true));
        }
    }
}

let last_fired = Rc::new(Cell::new(false));

let mut cap_in_clos = CapturedInClosure::default();

let timer_to_stop = Timer::default();
timer_to_stop.start(TimerMode::Repeated, Duration::from_millis(100), {
    let last_fired = last_fired.clone();
    move || {
    cap_in_clos.last_fired = Some(last_fired.clone());
}});

assert_eq!(last_fired.get(), false);
i_slint_backend_testing::mock_elapsed_time(110);
assert_eq!(last_fired.get(), false);
drop(timer_to_stop);

i_slint_backend_testing::mock_elapsed_time(110);
assert_eq!(last_fired.get(), true);
```
 */
#[cfg(doctest)]
const _TIMER_CLOSURE_DROP_STARTS_NEW_TIMER: () = ();

/**
 * Test that it's possible to set a timer's interval from within the callback.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    // Note: state will be leaked because of circular dependencies: don't do that in production
    timer: Timer,
    variable1: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
let state_ = state.clone();
state.borrow().timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().variable1 += 1;
    let variable1 = state_.borrow().variable1;
    if variable1 == 2 {
        state_.borrow().timer.set_interval(Duration::from_millis(500));
    } else if variable1 == 3 {
        state_.borrow().timer.set_interval(Duration::from_millis(100));
    }
});

assert!(state.borrow().timer.running());
i_slint_backend_testing::mock_elapsed_time(10);
assert!(state.borrow().timer.running());
assert_eq!(state.borrow().variable1, 0);
i_slint_backend_testing::mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1); // fired
assert!(state.borrow().timer.running());
i_slint_backend_testing::mock_elapsed_time(180);
assert_eq!(state.borrow().variable1, 1);
assert!(state.borrow().timer.running());
i_slint_backend_testing::mock_elapsed_time(30);
assert_eq!(state.borrow().variable1, 2); // fired
assert!(state.borrow().timer.running());
// now the timer interval should be 500
i_slint_backend_testing::mock_elapsed_time(480);
assert_eq!(state.borrow().variable1, 2);
assert!(state.borrow().timer.running());
i_slint_backend_testing::mock_elapsed_time(30);
assert_eq!(state.borrow().variable1, 3); // fired
assert!(state.borrow().timer.running());
// now the timer interval should be 100
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 4); // fired
assert!(state.borrow().timer.running());
i_slint_backend_testing::mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 5); // fired
assert!(state.borrow().timer.running());
```
 */
#[cfg(doctest)]
const _BUG6141_SET_INTERVAL_FROM_CALLBACK: () = ();

/**
 * Test that a timer can't be activated twice.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::Cell, time::Duration};

let later_timer_expiration_count = Rc::new(Cell::new(0));

let sooner_timer = Timer::default();
let later_timer = Rc::new(Timer::default());
later_timer.start(TimerMode::SingleShot, Duration::from_millis(500), {
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
        later_timer_expiration_count.set(later_timer_expiration_count.get() + 1);
    }
});

sooner_timer.start(TimerMode::SingleShot, Duration::from_millis(100), {
    let later_timer = later_timer.clone();
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
    later_timer.start(TimerMode::SingleShot, Duration::from_millis(600), {
        let later_timer_expiration_count = later_timer_expiration_count.clone();
        move || {
            later_timer_expiration_count.set(later_timer_expiration_count.get() + 1);
        }
    });
}});

assert_eq!(later_timer_expiration_count.get(), 0);
i_slint_backend_testing::mock_elapsed_time(110);
assert_eq!(later_timer_expiration_count.get(), 0);
i_slint_backend_testing::mock_elapsed_time(400);
assert_eq!(later_timer_expiration_count.get(), 0);
i_slint_backend_testing::mock_elapsed_time(800);
assert_eq!(later_timer_expiration_count.get(), 1);
```
 */
#[cfg(doctest)]
const _DOUBLY_REGISTER_ACTIVE_TIMER: () = ();

/**
 * Test that a timer can't be activated twice.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::Cell, time::Duration};

let later_timer_expiration_count = Rc::new(Cell::new(0));

let sooner_timer = Timer::default();
let later_timer = Rc::new(Timer::default());
later_timer.start(TimerMode::Repeated, Duration::from_millis(110), {
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
        later_timer_expiration_count.set(later_timer_expiration_count.get() + 1);
    }
});

sooner_timer.start(TimerMode::SingleShot, Duration::from_millis(100), {
    let later_timer = later_timer.clone();
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
    later_timer.start(TimerMode::Repeated, Duration::from_millis(110), {
        let later_timer_expiration_count = later_timer_expiration_count.clone();
        move || {
            later_timer_expiration_count.set(later_timer_expiration_count.get() + 1);
        }
    });
}});

assert_eq!(later_timer_expiration_count.get(), 0);
i_slint_backend_testing::mock_elapsed_time(120);
assert_eq!(later_timer_expiration_count.get(), 1);
```
 */
#[cfg(doctest)]
const _DOUBLY_REGISTER_ACTIVE_TIMER_2: () = ();

/**
 * Test that a timer that's being activated can be restarted and dropped in one go.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{cell::RefCell, rc::Rc, time::Duration};

let destructive_timer = Rc::new(RefCell::new(Some(Timer::default())));

destructive_timer.borrow().as_ref().unwrap().start(TimerMode::Repeated, Duration::from_millis(110), {
    let destructive_timer = destructive_timer.clone();
    move || {
        // start() used to reset the `being_activated` flag...
        destructive_timer.borrow().as_ref().unwrap().start(TimerMode::Repeated, Duration::from_millis(110), || {});
        // ... which would make this drop remove the timer from the timer list altogether and continued processing
        // of the timer would panic as the id isn't valid anymore.
        drop(destructive_timer.take());
    }
});

drop(destructive_timer);
i_slint_backend_testing::mock_elapsed_time(120);
```
 */
#[cfg(doctest)]
const _RESTART_TIMER_BEING_ACTIVATED: () = ();

/**
 * Test that a future timer can be stopped from the activation callback of an earlier timer.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::Cell, time::Duration};

let later_timer_expiration_count = Rc::new(Cell::new(0));

let sooner_timer = Timer::default();
let later_timer = Rc::new(Timer::default());
later_timer.start(TimerMode::SingleShot, Duration::from_millis(500), {
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
        later_timer_expiration_count.set(later_timer_expiration_count.get() + 1);
    }
});

sooner_timer.start(TimerMode::SingleShot, Duration::from_millis(100), {
    let later_timer = later_timer.clone();
    let later_timer_expiration_count = later_timer_expiration_count.clone();
    move || {
        later_timer.stop();
    }
});

assert_eq!(later_timer_expiration_count.get(), 0);
assert!(later_timer.running());
i_slint_backend_testing::mock_elapsed_time(110);
assert_eq!(later_timer_expiration_count.get(), 0);
assert!(!later_timer.running());
i_slint_backend_testing::mock_elapsed_time(800);
assert_eq!(later_timer_expiration_count.get(), 0);
assert!(!later_timer.running());
i_slint_backend_testing::mock_elapsed_time(800);
i_slint_backend_testing::mock_elapsed_time(800);
assert_eq!(later_timer_expiration_count.get(), 0);
```
 */
#[cfg(doctest)]
const _STOP_FUTURE_TIMER_DURING_ACTIVATION_OF_EARLIER: () = ();

/**
 * Test for issue #8897
```rust
use slint::TimerMode;
static DROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CALL1_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CALL2_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
std::thread::spawn(move || {
    struct StartTimerInDrop{};
    impl Drop for StartTimerInDrop {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
                println!("Timer fired");
            });
            let timer = slint::Timer::default();
            timer.start(TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
                println!("fired");
            });
            timer.restart();
            timer.stop();
        }
    }

    thread_local! { static START_TIMER_IN_DROP: StartTimerInDrop = StartTimerInDrop {}; }
    let timer = START_TIMER_IN_DROP.with(|_| { });
    thread_local! { static TIMER2: slint::Timer = slint::Timer::default(); }
    TIMER2.with(|timer| {
        timer.start(TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
            CALL2_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
    });


    i_slint_backend_testing::init_no_event_loop();
    slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
            CALL1_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    i_slint_backend_testing::mock_elapsed_time(50);

    assert_eq!(CALL1_COUNT.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(CALL2_COUNT.load(std::sync::atomic::Ordering::SeqCst), 0);
    i_slint_backend_testing::mock_elapsed_time(60);
    assert_eq!(CALL1_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(CALL2_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
    i_slint_backend_testing::mock_elapsed_time(60);
}).join().unwrap();
assert_eq!(DROP_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
assert_eq!(CALL1_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
assert_eq!(CALL2_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
```
 */
#[cfg(doctest)]
const _TIMER_AT_EXIT: () = ();

/**
 * A timer created from a context registers on that context: driving the current (global)
 * context leaves it alone, and driving its own fires it.
```rust
use i_slint_core::platform::*;
struct DummyBackend;
impl Platform for DummyBackend {
    fn create_window_adapter(&self) -> Result<std::rc::Rc<dyn WindowAdapter>, PlatformError> {
        Err(PlatformError::Other("not implemented".into()))
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(0)
    }
}

// Establishes the *global* context for this thread.
i_slint_backend_testing::init_no_event_loop();
// ... and a second one, which never becomes current.
let ctx = i_slint_core::SlintContext::new(Box::new(DummyBackend));

let fired = std::rc::Rc::new(std::cell::Cell::new(0));
let fired_ = fired.clone();
let timer = ctx.new_timer();
timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
    fired_.set(fired_.get() + 1);
});

// It is not in the global context's list, so driving that one does nothing...
i_slint_backend_testing::mock_elapsed_time(500);
assert_eq!(fired.get(), 0);
assert_eq!(i_slint_core::timers::TimerList::next_timeout(), None);

// ... while driving its own context fires it.
assert!(ctx.next_timer_timeout().is_some());
assert!(ctx.maybe_activate_timers(i_slint_core::animations::Instant(10_000)));
assert_eq!(fired.get(), 1);
```
 */
#[cfg(doctest)]
const _TIMER_ON_ITS_OWN_CONTEXT: () = ();

/**
 * A timer outliving its context goes inert rather than indexing a list that no longer
 * exists, and can be restarted onto the current context afterwards.
```rust
use i_slint_core::platform::*;
struct DummyBackend;
impl Platform for DummyBackend {
    fn create_window_adapter(&self) -> Result<std::rc::Rc<dyn WindowAdapter>, PlatformError> {
        Err(PlatformError::Other("not implemented".into()))
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(0)
    }
}
i_slint_backend_testing::init_no_event_loop();

let fired = std::rc::Rc::new(std::cell::Cell::new(0));
let fired_ = fired.clone();
let timer = {
    let ctx = i_slint_core::SlintContext::new(Box::new(DummyBackend));
    let timer = ctx.new_timer();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
        fired_.set(fired_.get() + 1);
    });
    assert!(timer.running());
    timer
}; // the context, and with it the list holding the callback, is dropped here

assert!(!timer.running());
assert_eq!(timer.interval(), std::time::Duration::default());
timer.stop();
timer.restart();
i_slint_backend_testing::mock_elapsed_time(500);
assert_eq!(fired.get(), 0);

// Starting it again moves it to the current context.
let fired_ = fired.clone();
timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
    fired_.set(fired_.get() + 1);
});
i_slint_backend_testing::mock_elapsed_time(50);
assert_eq!(fired.get(), 1);

drop(timer); // must not panic
```
 */
#[cfg(doctest)]
const _TIMER_OUTLIVING_ITS_CONTEXT: () = ();
