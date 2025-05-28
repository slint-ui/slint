// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore singleshot

#pragma once

#include <chrono>
#include <slint_timer_internal.h>

#ifndef SLINT_FEATURE_FREESTANDING
#    include <thread>
#    include <iostream>
#endif

namespace slint {

namespace private_api {
/// Internal function that checks that the API that must be called from the main
/// thread is indeed called from the main thread, or abort the program otherwise
///
/// Most API should be called from the main thread. When using thread one must
/// use slint::invoke_from_event_loop
inline void assert_main_thread()
{
#ifndef SLINT_FEATURE_FREESTANDING
#    ifndef NDEBUG
    static auto main_thread_id = std::this_thread::get_id();
    if (main_thread_id != std::this_thread::get_id()) {
        std::cerr << "A function that should be only called from the main thread was called from a "
                     "thread."
                  << std::endl;
        std::cerr << "Most API should be called from the main thread. When using thread one must "
                     "use slint::invoke_from_event_loop."
                  << std::endl;
        std::abort();
    }
#    endif
#endif
}
} // namespace private_api

using cbindgen_private::TimerMode;

/// A Timer that can call a callback at repeated interval
///
/// Use the static single_shot function to make a single shot timer
struct Timer
{
    /// Construct a null timer. Use the start() method to activate the timer with a mode, interval
    /// and callback.
    Timer() = default;
    /// Construct a timer which will repeat the callback every `interval` milliseconds until
    /// the destructor of the timer is called.
    ///
    /// This is a convenience function and equivalent to calling
    /// `start(slint::TimerMode::Repeated, interval, callback);` on a default constructed Timer.
    template<std::invocable F>
    Timer(std::chrono::milliseconds interval, F callback)
        : id(cbindgen_private::slint_timer_start(
                  0, TimerMode::Repeated, interval.count(),
                  [](void *data) { (*reinterpret_cast<F *>(data))(); }, new F(std::move(callback)),
                  [](void *data) { delete reinterpret_cast<F *>(data); }))
    {
        private_api::assert_main_thread();
    }
    Timer(const Timer &) = delete;
    Timer &operator=(const Timer &) = delete;
    ~Timer()
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_timer_destroy(id);
    }

    /// Starts the timer with the given \a mode and \a interval, in order for the \a callback to
    /// called when the timer fires. If the timer has been started previously and not fired yet,
    /// then it will be restarted.
    template<std::invocable F>
    void start(TimerMode mode, std::chrono::milliseconds interval, F callback)
    {
        private_api::assert_main_thread();
        id = cbindgen_private::slint_timer_start(
                id, mode, interval.count(), [](void *data) { (*reinterpret_cast<F *>(data))(); },
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }
    /// Stops the previously started timer. Does nothing if the timer has never been started. A
    /// stopped timer cannot be restarted with restart(). Use start() instead.
    void stop()
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_timer_stop(id);
    }
    /// Restarts the timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    void restart()
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_timer_restart(id);
    }
    /// Returns true if the timer is running; false otherwise.
    bool running() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_timer_running(id);
    }
    /// Returns the interval of the timer.
    /// Returns 0 if the timer was never started.
    std::chrono::milliseconds interval() const
    {
        private_api::assert_main_thread();
        return std::chrono::milliseconds(cbindgen_private::slint_timer_interval(id));
    }

    /// Call the callback after the given duration.
    template<std::invocable F>
    static void single_shot(std::chrono::milliseconds duration, F callback)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_timer_singleshot(
                duration.count(), [](void *data) { (*reinterpret_cast<F *>(data))(); },
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }

private:
    uint64_t id = 0;
};

} // namespace slint
