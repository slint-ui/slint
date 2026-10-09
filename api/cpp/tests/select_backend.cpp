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

TEST_CASE("select_backend")
{
    using GraphicsAPI = slint::BackendRequirements::GraphicsAPI;

    auto error = slint::select_backend({ .backend = "nonexistent" });
    REQUIRE(error.has_value());
    REQUIRE(std::string_view(*error).find("nonexistent") != std::string_view::npos);

    REQUIRE(slint::select_backend(
                    { .graphics_api = GraphicsAPI::Metal, .min_version = { { 1, 0 } } })
                    .has_value());

    slint::platform::set_platform(std::make_unique<TestPlatform>());
    REQUIRE(slint::select_backend(
                    { .graphics_api = GraphicsAPI::OpenGL, .min_version = { { 3, 2 } } })
                    .has_value());
}
