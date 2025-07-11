// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>
#include <slint-interpreter.h>
#include <slint-testing.h>

SCENARIO("ElementHandle")
{
    using namespace slint::interpreter;
    using namespace slint;

    ComponentCompiler compiler;

    auto result = compiler.build_from_source(
            R"(
        component ButtonBase {
            @children
        }
        component PushButton inherits ButtonBase {
            accessible-role: button;
            in property <string> text <=> label.text;
            label := Text {}
        }
        export component App {
            VerticalLayout {
                PushButton { text: "first"; }
                second := PushButton { text: "second"; }
            }
        }
    )",
            "");
    for (auto &&x : compiler.diagnostics())
        std::cerr << x.message << std::endl;
    REQUIRE(result.has_value());
    auto component_definition = *result;

    auto instance = component_definition.create();

    SECTION("Find by accessible label")
    {
        auto elements = slint::testing::ElementHandle::find_by_accessible_label(instance, "first");
        REQUIRE(elements.size() == 1);
        REQUIRE(*elements[0].accessible_label() == "first");
        REQUIRE(*elements[0].id() == "PushButton::label");
        REQUIRE(*elements[0].type_name() == "Text");
        REQUIRE((*elements[0].bases()).size() == 0);
    }

    SECTION("Find by id")
    {
        auto elements = slint::testing::ElementHandle::find_by_element_id(instance, "App::second");
        REQUIRE(elements.size() == 1);
        REQUIRE(*elements[0].id() == "App::second");
        REQUIRE(*elements[0].type_name() == "PushButton");
        REQUIRE((*elements[0].bases()).size() == 1);
        REQUIRE((*elements[0].bases())[0] == "ButtonBase");
        REQUIRE(*elements[0].accessible_role() == slint::testing::AccessibleRole::Button);
    }

    SECTION("Find by type name")
    {
        auto elements =
                slint::testing::ElementHandle::find_by_element_type_name(instance, "PushButton");
        REQUIRE(elements.size() == 2);
        REQUIRE(*elements[0].id() == "");
        REQUIRE(*elements[1].id() == "App::second");
    }
}
