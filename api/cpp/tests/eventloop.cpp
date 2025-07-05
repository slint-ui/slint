// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore singleshot

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>
#include <thread>

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
    REQUIRE(timer.running());

    bool timer_was_running = true;

    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(!timer.running());
    REQUIRE(timer_triggered == 1);
    REQUIRE(!timer_was_running); // At that point the timer is already considered stopped!

    timer_triggered = 0;
    timer_was_running = true;

    timer.restart();
    REQUIRE(timer.running());
    slint::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        slint::quit_event_loop();
    });

    slint::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(!timer_was_running);
    REQUIRE(!timer.running());
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
