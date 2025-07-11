// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>
#include <slint_image.h>

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
