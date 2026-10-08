// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint-platform.h>

struct TestPlatform : slint::platform::Platform
{
    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override
    {
        return nullptr;
    }
};

TEST_CASE("BackendSelector")
{
    auto error = slint::BackendSelector().backend_name("nonexistent").select();
    REQUIRE(error.has_value());
    REQUIRE(std::string_view(*error).find("nonexistent") != std::string_view::npos);

    slint::platform::set_platform(std::make_unique<TestPlatform>());
    REQUIRE(slint::BackendSelector().require_opengl_with_version(3, 2).select().has_value());
}
