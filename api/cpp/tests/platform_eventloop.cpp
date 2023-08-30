// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <slint-platform.h>
#include <thread>
#include <deque>
#include <memory>
#include <mutex>
#include <chrono>
#include <optional>

struct TestPlatform : slint::platform::Platform
{
    std::mutex the_mutex;
    std::deque<slint::platform::Platform::Task> queue;
    bool quit = false;
    std::condition_variable cv;
    std::chrono::time_point<std::chrono::steady_clock> start = std::chrono::steady_clock::now();

    /// Returns a new WindowAdapter
    virtual std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override
    {
        assert(!"creating window in this test");
        return nullptr;
    };

    /// Spins an event loop and renders the visible windows.
    virtual void run_event_loop() override
    {
        while (true) {
            slint::platform::update_timers_and_animations();
            std::optional<slint::platform::Platform::Task> event;
            {
                std::unique_lock lock(the_mutex);
                if (queue.empty()) {
                    if (quit) {
                        quit = false;
                        break;
                    }
                    if (auto duration = slint::platform::duration_until_next_timer_update()) {
                        cv.wait_for(lock, *duration);
                    } else {
                        cv.wait(lock);
                    }
                    continue;
                } else {
                    event = std::move(queue.front());
                    queue.pop_front();
                }
            }
            if (event) {
                std::move(*event).run();
                event.reset();
            }
        }
    }

    virtual void quit_event_loop() override
    {
        const std::unique_lock lock(the_mutex);
        quit = true;
        cv.notify_all();
    }

    virtual void run_in_event_loop(slint::platform::Platform::Task event) override
    {
        const std::unique_lock lock(the_mutex);
        queue.push_back(std::move(event));
        cv.notify_all();
    }

#ifdef SLINT_FEATURE_FREESTANDING
    virtual std::chrono::milliseconds duration_since_start() override
    {
        return std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now() - start);
    }
#endif
};

bool init_platform = (slint::platform::set_platform(std::make_unique<TestPlatform>()), true);

TEST_CASE("C++ Singleshot Timers")
{
    using namespace slint;
    int called = 0;
    Timer testTimer(std::chrono::milliseconds(16), [&]() {
        slint::quit_event_loop();
        called += 10;
    });
    REQUIRE(called == 0);
    slint::run_event_loop();
    REQUIRE(called == 10);
}

TEST_CASE("C++ Repeated Timer")
{
    int timer_triggered = 0;
    slint::Timer timer;

    timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("C++ Restart Singleshot Timer")
{
    int timer_triggered = 0;
    slint::Timer timer;

    timer.start(slint::TimerMode::SingleShot, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(timer_was_running);
    timer_triggered = 0;
    timer.restart();
    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("C++ Restart Repeated Timer")
{
    int timer_triggered = 0;
    slint::Timer timer;

    timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;
    timer.stop();
    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 0);
    REQUIRE(!timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;

    timer.restart();

    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("Quit from event")
{
    int called = 0;
    slint::invoke_from_event_loop([&] {
        slint::quit_event_loop();
        called += 10;
    });
    REQUIRE(called == 0);
    slint::run_event_loop();
    REQUIRE(called == 10);
}

TEST_CASE("Event from thread")
{
    std::atomic<int> called = 0;
    auto t = std::thread([&] {
        called += 10;
        slint::invoke_from_event_loop([&] {
            called += 100;
            slint::quit_event_loop();
        });
    });

    slint::run_event_loop();
    REQUIRE(called == 110);
    t.join();
}

TEST_CASE("Blocking Event from thread")
{
    std::atomic<int> called = 0;
    auto t = std::thread([&] {
        // test returning a, unique_ptr because it is movable-only
        std::unique_ptr foo =
                slint::blocking_invoke_from_event_loop([&] { return std::make_unique<int>(42); });
        called = *foo;
        int xxx = 123;
        slint::blocking_invoke_from_event_loop([&] {
            slint::quit_event_loop();
            xxx = 888999;
        });
        REQUIRE(xxx == 888999);
    });

    slint::run_event_loop();
    REQUIRE(called == 42);
    t.join();
}
