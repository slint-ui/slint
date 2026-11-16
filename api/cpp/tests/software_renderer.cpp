// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint-interpreter.h>
#include <slint-platform.h>

#include <iostream>
#include <vector>

namespace {

constexpr std::size_t WIDTH = 4;
constexpr std::size_t HEIGHT = 2;

struct TestWindowAdapter : slint::platform::WindowAdapter
{
    slint::platform::SoftwareRenderer software_renderer {
        slint::platform::SoftwareRenderer::RepaintBufferType::NewBuffer
    };
    slint::PhysicalSize size() override { return slint::PhysicalSize({ WIDTH, HEIGHT }); }
    slint::platform::AbstractRenderer &renderer() override { return software_renderer; }
};

TestWindowAdapter *the_window_adapter = nullptr;

struct TestPlatform : slint::platform::Platform
{
    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override
    {
        auto adapter = std::make_unique<TestWindowAdapter>();
        the_window_adapter = adapter.get();
        return adapter;
    }
};

bool init_platform = (slint::platform::set_platform(std::make_unique<TestPlatform>()), true);

/// The left half is covered with a half transparent red rectangle, the right half stays
/// transparent.
slint::ComponentHandle<slint::interpreter::ComponentInstance> create_scene()
{
    slint::interpreter::ComponentCompiler compiler;
    auto definition = compiler.build_from_source(R"(
        export component Scene inherits Window {
            width: 4px;
            height: 2px;
            background: transparent;
            Rectangle {
                x: 0px;
                y: 0px;
                width: 2px;
                height: 2px;
                background: #ff000080;
            }
        }
    )",
                                                 "");
    for (auto &diagnostic : compiler.diagnostics()) {
        std::cerr << std::string_view(diagnostic.message) << std::endl;
    }
    REQUIRE(definition.has_value());
    auto instance = definition->create();
    instance->show();
    REQUIRE(the_window_adapter != nullptr);
    return instance;
}

void check_pixels(const std::vector<slint::PremultipliedRgba8Pixel> &buffer)
{
    const slint::PremultipliedRgba8Pixel red { 0x80, 0, 0, 0x80 };
    const slint::PremultipliedRgba8Pixel transparent { 0, 0, 0, 0 };
    for (std::size_t y = 0; y < HEIGHT; ++y) {
        for (std::size_t x = 0; x < WIDTH; ++x) {
            INFO("pixel " << x << "," << y);
            const auto &pixel = buffer[y * WIDTH + x];
            const auto &expected = x < 2 ? red : transparent;
            CHECK(int(pixel.r) == int(expected.r));
            CHECK(int(pixel.g) == int(expected.g));
            CHECK(int(pixel.b) == int(expected.b));
            CHECK(int(pixel.a) == int(expected.a));
        }
    }
}

}

TEST_CASE("C++ software renderer: premultiplied RGBA buffer")
{
    auto instance = create_scene();
    // Start with garbage to check that dirty regions are cleared to transparent.
    std::vector<slint::PremultipliedRgba8Pixel> buffer(WIDTH * HEIGHT, { 1, 2, 3, 4 });
    the_window_adapter->software_renderer.render(std::span(buffer), WIDTH);
    check_pixels(buffer);
}

#ifdef SLINT_FEATURE_EXPERIMENTAL
TEST_CASE("C++ software renderer: premultiplied RGBA TargetPixelBuffer")
{
    // A buffer whose hooks decline everything, so the renderer draws through line_slice().
    struct Buffer
        : slint::platform::SoftwareRenderer::TargetPixelBuffer<slint::PremultipliedRgba8Pixel>
    {
        std::vector<slint::PremultipliedRgba8Pixel> pixels =
                std::vector<slint::PremultipliedRgba8Pixel>(WIDTH * HEIGHT, { 1, 2, 3, 4 });
        int fill_background_calls = 0;
        int draw_rectangle_calls = 0;

        std::span<slint::PremultipliedRgba8Pixel> line_slice(std::size_t line) override
        {
            return std::span(pixels).subspan(line * WIDTH, WIDTH);
        }
        std::size_t num_lines() override { return HEIGHT; }
        bool draw_texture(const slint::platform::SoftwareRenderer::DrawTextureArgs &,
                          const slint::platform::SoftwareRenderer::PhysicalRegion &) override
        {
            return false;
        }
        bool fill_background(const slint::Brush &,
                             const slint::platform::SoftwareRenderer::PhysicalRegion &) override
        {
            ++fill_background_calls;
            return false;
        }
        bool draw_rectangle(const slint::platform::SoftwareRenderer::DrawRectangleArgs &,
                            const slint::platform::SoftwareRenderer::PhysicalRegion &) override
        {
            ++draw_rectangle_calls;
            return false;
        }
    };

    auto instance = create_scene();
    Buffer buffer;
    the_window_adapter->software_renderer.render(&buffer);
    CHECK(buffer.fill_background_calls > 0);
    CHECK(buffer.draw_rectangle_calls > 0);
    check_pixels(buffer.pixels);
}
#endif
