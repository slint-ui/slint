// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <sixtyfps.h>
#include <thread>

TEST_CASE("C++ Singleshot Timers")
{
    using namespace sixtyfps;
    int called = 0;
    Timer testTimer(std::chrono::milliseconds(16), [&]() {
        sixtyfps::quit_event_loop();
        called += 10;
    });
    REQUIRE(called == 0);
    sixtyfps::run_event_loop();
    REQUIRE(called == 10);
}

TEST_CASE("C++ Repeated Timer")
{
    int timer_triggered = 0;
    sixtyfps::Timer timer;

    timer.start(sixtyfps::TimerMode::Repeated, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("C++ Restart Singleshot Timer")
{
    int timer_triggered = 0;
    sixtyfps::Timer timer;

    timer.start(sixtyfps::TimerMode::SingleShot, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(timer_was_running);
    timer_triggered = 0;
    timer.restart();
    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered == 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("C++ Restart Repeated Timer")
{
    int timer_triggered = 0;
    sixtyfps::Timer timer;

    timer.start(sixtyfps::TimerMode::Repeated, std::chrono::milliseconds(30),
                [&]() { timer_triggered++; });

    REQUIRE(timer_triggered == 0);

    bool timer_was_running = false;

    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;
    timer.stop();
    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered == 0);
    REQUIRE(!timer_was_running);

    timer_was_running = false;
    timer_triggered = 0;

    timer.restart();

    sixtyfps::Timer::single_shot(std::chrono::milliseconds(500), [&]() {
        timer_was_running = timer.running();
        sixtyfps::quit_event_loop();
    });

    sixtyfps::run_event_loop();

    REQUIRE(timer_triggered > 1);
    REQUIRE(timer_was_running);
}

TEST_CASE("Quit from event")
{
    int called = 0;
    sixtyfps::invoke_from_event_loop([&] {
        sixtyfps::quit_event_loop();
        called += 10;
    });
    REQUIRE(called == 0);
    sixtyfps::run_event_loop();
    REQUIRE(called == 10);
}

TEST_CASE("Event from thread")
{
    std::atomic<int> called = 0;
    auto t = std::thread([&] {
        called += 10;
        sixtyfps::invoke_from_event_loop([&] {
            called += 100;
            sixtyfps::quit_event_loop();
        });
    });

    sixtyfps::run_event_loop();
    REQUIRE(called == 110);
    t.join();
}

TEST_CASE("Blocking Event from thread")
{
    std::atomic<int> called = 0;
    auto t = std::thread([&] {
        // test returning a, unique_ptr because it is movable-only
        std::unique_ptr foo = sixtyfps::blocking_invoke_from_event_loop(
                [&] { return std::make_unique<int>(42); });
        called = *foo;
        int xxx = 123;
        sixtyfps::blocking_invoke_from_event_loop([&] {
            sixtyfps::quit_event_loop();
            xxx = 888999;
        });
        REQUIRE(xxx == 888999);
    });

    sixtyfps::run_event_loop();
    REQUIRE(called == 42);
    t.join();
}
