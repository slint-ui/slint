// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <slint.h>
#include <thread>

#include <slint_interpreter.h>

TEST_CASE("Basic Window Visibility")
{
    using namespace slint::interpreter;
    using namespace slint;

    ComponentCompiler compiler;
    auto comp_def = compiler.build_from_source(R"(
        export App := Window {
        }
    )",
                                               "");
    REQUIRE(comp_def.has_value());
    auto instance = comp_def->create();
    REQUIRE(instance->window().is_visible() == false);
    instance->show();
    REQUIRE(instance->window().is_visible() == true);
    instance->hide();
    REQUIRE(instance->window().is_visible() == false);
}
