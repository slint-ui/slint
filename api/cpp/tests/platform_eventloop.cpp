// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore singleshot

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

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
#ifdef SLINT_FEATURE_RENDERER_SOFTWARE
        struct TestWindowAdapter : slint::platform::WindowAdapter
        {
            slint::platform::SoftwareRenderer r { { } };
            slint::PhysicalSize size() override { return slint::PhysicalSize({}); }
            slint::platform::AbstractRenderer &renderer() override { return r; }
        };
        return std::make_unique<TestWindowAdapter>();
#else
        assert(!"creating window in this test");
        return nullptr;
#endif
    };

    /// Spins an event loop and renders the visible windows.
    virtual void run_event_loop() override
    {
        quit = false;
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

    timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(3),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    slint::Timer::single_shot(std::chrono::milliseconds(100), [&]() {
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

    timer.start(slint::TimerMode::SingleShot, std::chrono::milliseconds(3),
                [&]() { timer_triggered++; });
    REQUIRE(timer.running());

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = true;

    slint::Timer::single_shot(std::chrono::milliseconds(50), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(!timer_was_running); // Timer is already stopped at this point

    timer_was_running = true;
    timer_triggered = 0;
    timer.restart();
    REQUIRE(timer.running());
    slint::Timer::single_shot(std::chrono::milliseconds(50), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(!timer_was_running);
}

TEST_CASE("C++ Restart Repeated Timer")
{
    int timer_triggered = 0;
    slint::Timer timer;

    timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(3),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    slint::Timer::single_shot(std::chrono::milliseconds(50), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;
    timer.stop();
    slint::Timer::single_shot(std::chrono::milliseconds(50), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 0);
    REQUIRE(!timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;

    timer.restart();

    slint::Timer::single_shot(std::chrono::milliseconds(50), [&]() {
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

#if defined(SLINT_FEATURE_INTERPRETER) && defined(SLINT_FEATURE_RENDERER_SOFTWARE)

#    include <slint-interpreter.h>

TEST_CASE("Quit on last window closed")
{
    using namespace slint::interpreter;
    using namespace slint;

    int ok = 0;

    ComponentCompiler compiler;
    auto comp_def = compiler.build_from_source("export component App inherits Window { }", "");
    REQUIRE(comp_def.has_value());
    auto instance = comp_def->create();
    instance->hide(); // hide before show should mess the counter
    REQUIRE(instance->window().is_visible() == false);
    instance->show();
    REQUIRE(instance->window().is_visible() == true);

    slint::Timer::single_shot(std::chrono::milliseconds(10), [&]() {
        REQUIRE(instance->window().is_visible() == true);
        instance->hide();
        REQUIRE(instance->window().is_visible() == false);
        ok = 1;
        slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
            // event loop should be stopped
            ok = -1;
        });
    });
    slint::run_event_loop();
    REQUIRE(ok == 1);
    REQUIRE(instance->window().is_visible() == false);

    ok = 0;
    slint::Timer::single_shot(std::chrono::milliseconds(5), [&]() {
        REQUIRE(ok == -1); // the event we started previously should have been ran first
        ok = 1;
        REQUIRE(instance->window().is_visible() == false);
        instance->show();
        instance->show(); // two show shouldn't make the loop alive
        slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
            REQUIRE(instance->window().is_visible() == true);
            instance->hide();
            ok = 2;
            slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
                // event loop should be stopped
                ok = -2;
            });
        });
    });
    slint::run_event_loop();
    REQUIRE(ok == 2);

    ok = 0;
    auto instance2 = comp_def->create();
    instance2->show();
    slint::Timer::single_shot(std::chrono::milliseconds(5), [&]() {
        REQUIRE(ok == -2); // the event we started previously should have been ran first
        instance->show();
        instance2->hide();
        slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
            instance2->show();
            instance->hide();
            slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
                instance2->hide();
                ok = 3;
            });
        });
    });
    slint::run_event_loop();
    REQUIRE(ok == 3);
    ok = 0;
    slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
        REQUIRE(ok == 0);
        instance->show();
        slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
            instance2->hide();
            instance->hide();
            slint::Timer::single_shot(std::chrono::milliseconds(0), [&]() {
                instance2->show();
                slint::quit_event_loop();
                ok = 4;
            });
        });
    });
    slint::run_event_loop(slint::EventLoopMode::RunUntilQuit);

    REQUIRE(ok == 4);
}

#endif
