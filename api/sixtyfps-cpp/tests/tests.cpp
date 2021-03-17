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
#include <sixtyfps_interpreter.h>

SCENARIO("SharedString API")
{
    sixtyfps::SharedString str;

    REQUIRE(str.empty());

    SECTION("Construct from string_view")
    {
        std::string foo("Foo");
        std::string_view foo_view(foo);
        str = foo_view;
        REQUIRE(str == "Foo");
    }
}

SCENARIO("Value API")
{
    using namespace sixtyfps::interpreter;
    Value value;

    REQUIRE(value.type() == Value::Type::Void);

    SECTION("Construct a string")
    {
        REQUIRE(!value.to_string().has_value());
        sixtyfps::SharedString cpp_str("Hello World");
        value = Value(cpp_str);
        REQUIRE(value.type() == Value::Type::String);

        auto string_opt = value.to_string();
        REQUIRE(string_opt.has_value());
        REQUIRE(string_opt.value() == "Hello World");
    }

    SECTION("Construct a number")
    {
        REQUIRE(!value.to_number().has_value());
        const double number = 42.0;
        value = Value(number);
        REQUIRE(value.type() == Value::Type::Number);

        auto number_opt = value.to_number();
        REQUIRE(number_opt.has_value());
        REQUIRE(number_opt.value() == number);
    }

    SECTION("Construct a bool")
    {
        REQUIRE(!value.to_bool().has_value());
        value = Value(true);
        REQUIRE(value.type() == Value::Type::Bool);

        auto bool_opt = value.to_bool();
        REQUIRE(bool_opt.has_value());
        REQUIRE(bool_opt.value() == true);
    }
}