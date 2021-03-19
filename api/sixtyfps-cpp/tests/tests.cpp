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

TEST_CASE("Basic SharedVector API", "[vector]")
{
    sixtyfps::SharedVector<int> vec;
    REQUIRE(vec.empty());

    SECTION("Initializer list")
    {
        sixtyfps::SharedVector<int> vec({ 1, 4, 10 });
        REQUIRE(vec.size() == 3);
        REQUIRE(vec[0] == 1);
        REQUIRE(vec[1] == 4);
        REQUIRE(vec[2] == 10);
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

    SECTION("Construct an array")
    {
        REQUIRE(!value.to_array().has_value());
        sixtyfps::SharedVector<Value> array { Value(42.0), Value(true) };
        value = Value(array);
        REQUIRE(value.type() == Value::Type::Array);

        auto array_opt = value.to_array();
        REQUIRE(array_opt.has_value());

        auto extracted_array = array_opt.value();
        REQUIRE(extracted_array.size() == 2);
        REQUIRE(extracted_array[0].to_number().value() == 42);
        REQUIRE(extracted_array[1].to_bool().value());
    }

    SECTION("Construct a brush")
    {
        REQUIRE(!value.to_brush().has_value());
        sixtyfps::Brush brush(sixtyfps::Color::from_rgb_uint8(255, 0, 255));
        value = Value(brush);
        REQUIRE(value.type() == Value::Type::Brush);

        auto brush_opt = value.to_brush();
        REQUIRE(brush_opt.has_value());
        REQUIRE(brush_opt.value() == brush);
    }

    SECTION("Construct a struct")
    {
        REQUIRE(!value.to_struct().has_value());
        sixtyfps::interpreter::Struct struc;
        value = Value(struc);
        REQUIRE(value.type() == Value::Type::Struct);

        auto struct_opt = value.to_struct();
        REQUIRE(struct_opt.has_value());
    }

    SECTION("Construct a model")
    {
        // And test that it is properly destroyed when the value is destroyed
        struct M : sixtyfps::VectorModel<Value>
        {
            bool *destroyed;
            explicit M(bool *destroyed) : destroyed(destroyed) { }
            void play()
            {
                this->push_back(Value(4.));
                this->set_row_data(0, Value(9.));
            }
            ~M() { *destroyed = true; }
        };
        bool destroyed = false;
        auto m = std::make_shared<M>(&destroyed);
        {
            Value value(m);
            REQUIRE(value.type() == Value::Type::Model);
            REQUIRE(!destroyed);
            m->play();
            m = nullptr;
            REQUIRE(!destroyed);
            // play a bit with the value to test the copy and move
            Value v2 = value;
            Value v3 = std::move(v2);
            REQUIRE(!destroyed);
        }
        REQUIRE(destroyed);
    }

    SECTION("Compare Values")
    {
        Value str1 { sixtyfps::SharedString("Hello1") };
        Value str2 { sixtyfps::SharedString("Hello2") };
        Value fl1 { 10. };
        Value fl2 { 12. };

        REQUIRE(str1 == str1);
        REQUIRE(str1 != str2);
        REQUIRE(str1 != fl2);
        REQUIRE(fl1 == fl1);
        REQUIRE(fl1 != fl2);
        REQUIRE(Value() == Value());
        REQUIRE(Value() != str1);
        REQUIRE(str1 == sixtyfps::SharedString("Hello1"));
        REQUIRE(str1 != sixtyfps::SharedString("Hello2"));
        REQUIRE(sixtyfps::SharedString("Hello2") == str2);
        REQUIRE(fl1 != sixtyfps::SharedString("Hello2"));
        REQUIRE(fl2 == 12.);
    }
}

SCENARIO("Struct API")
{
    using namespace sixtyfps::interpreter;
    Struct struc;

    REQUIRE(!struc.get_field("not_there"));

    struc.set_field("field_a", Value(sixtyfps::SharedString("Hallo")));

    auto value_opt = struc.get_field("field_a");
    REQUIRE(value_opt.has_value());
    auto value = value_opt.value();
    REQUIRE(value.to_string().has_value());
    REQUIRE(value.to_string().value() == "Hallo");

    int count = 0;
    for (auto [k, value] : struc) {
        REQUIRE(count == 0);
        count++;
        REQUIRE(k == "field_a");
        REQUIRE(value.to_string().value() == "Hallo");
    }

    struc.set_field("field_b", Value(sixtyfps::SharedString("World")));
    std::map<std::string, sixtyfps::SharedString> map;
    for (auto [k, value] : struc)
        map[std::string(k)] = *value.to_string();

    REQUIRE(map
            == std::map<std::string, sixtyfps::SharedString> {
                    { "field_a", sixtyfps::SharedString("Hallo") },
                    { "field_b", sixtyfps::SharedString("World") } });
}

SCENARIO("Struct Iterator Constructor")
{
    using namespace sixtyfps::interpreter;

    std::vector<std::pair<std::string_view, Value>> values = { { "field_a", Value(true) },
                                                               { "field_b", Value(42.0) } };

    Struct struc(values.begin(), values.end());

    REQUIRE(!struc.get_field("foo").has_value());
    REQUIRE(struc.get_field("field_a").has_value());
    REQUIRE(struc.get_field("field_a").value().to_bool().value());
    REQUIRE(struc.get_field("field_b").value().to_number().value() == 42.0);
}

SCENARIO("Struct Initializer List Constructor")
{
    using namespace sixtyfps::interpreter;

    Struct struc({ { "field_a", Value(true) }, { "field_b", Value(42.0) } });

    REQUIRE(!struc.get_field("foo").has_value());
    REQUIRE(struc.get_field("field_a").has_value());
    REQUIRE(struc.get_field("field_a").value().to_bool().value());
    REQUIRE(struc.get_field("field_b").value().to_number().value() == 42.0);
}

SCENARIO("Component Compiler")
{
    using namespace sixtyfps::interpreter;
    using namespace sixtyfps;

    ComponentCompiler compiler;

    SECTION("configure include paths")
    {
        SharedVector<SharedString> in_paths;
        in_paths.push_back("path1");
        in_paths.push_back("path2");
        compiler.set_include_paths(in_paths);

        auto out_paths = compiler.include_paths();
        REQUIRE(out_paths.size() == 2);
        REQUIRE(out_paths[0] == "path1");
        REQUIRE(out_paths[1] == "path2");
    }

    SECTION("Compile failure from source")
    {
        auto result = compiler.build_from_source("Syntax Error!!", "");
        REQUIRE_FALSE(result.has_value());
    }

    SECTION("Compile from source")
    {
        auto result = compiler.build_from_source("export Dummy := Rectangle {}", "");
        REQUIRE(result.has_value());
    }
}