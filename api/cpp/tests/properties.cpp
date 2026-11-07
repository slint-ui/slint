// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include <algorithm>
#include <array>
#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>

using slint::private_api::Property;

SCENARIO("Basic usage")
{
    Property<int> prop;
    REQUIRE(prop.get() == 0);

    prop.set(42);
    REQUIRE(prop.get() == 42);

    {
        Property<int> prop2;
        prop2.set_binding([&] { return prop.get() + 4; });
        REQUIRE(prop2.get() == 42 + 4);
        prop.set(55);
        REQUIRE(prop2.get() == 55 + 4);
    }

    REQUIRE(prop.get() == 55);
    prop.set(33);
    REQUIRE(prop.get() == 33);
}

SCENARIO("Set after binding")
{
    Property<int> prop;
    REQUIRE(prop.get() == 0);
    prop.set_binding([] { return 55; });
    prop.set(0);
    REQUIRE(prop.get() == 0);
}

SCENARIO("Linking properties that are already linked")
{
    Property<int> p1(1);
    Property<int> p2(2);
    Property<int>::link_two_way(&p1, &p2);
    Property<int>::link_two_way(&p1, &p2);
    Property<int>::link_two_way(&p2, &p1);
    REQUIRE(p1.get() == 2);
    p1.set(3);
    REQUIRE(p2.get() == 3);

    Property<int> p3(4);
    Property<int>::link_two_way(&p2, &p3);
    Property<int>::link_two_way(&p3, &p1);
    p3.set(5);
    REQUIRE(p1.get() == 5);
    REQUIRE(p2.get() == 5);
}

SCENARIO("Linking a property that has a binding")
{
    Property<int> p1(1);
    Property<int> p2(2);
    p2.set_binding([] { return 6; });
    Property<int>::link_two_way(&p1, &p2);
    REQUIRE(p1.get() == 6);

    Property<int> p3(3);
    p3.set_binding([] { return 7; });
    Property<int>::link_two_way(&p3, &p1);
    REQUIRE(p3.get() == 6);
    p3.set(8);
    REQUIRE(p2.get() == 8);
}

SCENARIO("Linking chains of properties in any order")
{
    const std::array<std::pair<int, int>, 4> links { { { 0, 1 }, { 2, 3 }, { 0, 2 }, { 1, 3 } } };
    std::array<int, 4> order { 0, 1, 2, 3 };
    do {
        for (int directions = 0; directions < 16; ++directions) {
            std::array<Property<int>, 4> props { Property<int>(0), Property<int>(1),
                                                 Property<int>(2), Property<int>(3) };
            for (int n = 0; n < 4; ++n) {
                auto [a, b] = links[order[n]];
                if (directions & (1 << n)) {
                    std::swap(a, b);
                }
                Property<int>::link_two_way(&props[a], &props[b]);
            }
            for (int value = 0; value < 4; ++value) {
                props[value].set(value + 100);
                for (const auto &p : props) {
                    REQUIRE(p.get() == value + 100);
                }
            }
        }
    } while (std::next_permutation(order.begin(), order.end()));
}

SCENARIO("Linking to a linked property keeps the binding")
{
    Property<int> source(1);
    Property<int> p1(0);
    Property<int> p2(0);
    Property<int> other(0);
    Property<int>::link_two_way(&p1, &other);
    p2.set_binding([&] { return source.get() + 1; });
    Property<int>::link_two_way(&p1, &p2);
    REQUIRE(p1.get() == 2);
    source.set(5);
    REQUIRE(p1.get() == 6);
    REQUIRE(other.get() == 6);
}
