/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <sixtyfps.h>
#include <thread>

TEST_CASE("C++ Timers")
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


SCENARIO("Quit from event")
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


SCENARIO("Event from thread")
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
