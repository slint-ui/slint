/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <sixtyfps.h>
#include <sixtyfps_image.h>

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

    SECTION("Construct from char*")
    {
        str = "Bar";
        REQUIRE(str == "Bar");
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

TEST_CASE("Property Tracker")
{
    using namespace sixtyfps::private_api;
    PropertyTracker tracker1;
    PropertyTracker tracker2;
    Property<int> prop(42);

    auto r = tracker1.evaluate([&]() { return tracker2.evaluate([&]() { return prop.get(); }); });
    REQUIRE(r == 42);

    prop.set(1);
    REQUIRE(tracker2.is_dirty());
    REQUIRE(tracker1.is_dirty());

    r = tracker1.evaluate(
            [&]() { return tracker2.evaluate_as_dependency_root([&]() { return prop.get(); }); });
    REQUIRE(r == 1);
    prop.set(100);
    REQUIRE(tracker2.is_dirty());
    REQUIRE(!tracker1.is_dirty());
}

TEST_CASE("Model row changes")
{
    using namespace sixtyfps::private_api;

    auto model = std::make_shared<sixtyfps::VectorModel<int>>();

    PropertyTracker tracker;

    REQUIRE(tracker.evaluate([&]() {
        model->track_row_count_changes();
        return model->row_count();
    }) == 0);
    REQUIRE(!tracker.is_dirty());
    model->push_back(1);
    model->push_back(2);
    REQUIRE(tracker.is_dirty());
    REQUIRE(tracker.evaluate([&]() {
        model->track_row_count_changes();
        return model->row_count();
    }) == 2);
    REQUIRE(!tracker.is_dirty());
    model->erase(0);
    REQUIRE(tracker.is_dirty());
    REQUIRE(tracker.evaluate([&]() {
        model->track_row_count_changes();
        return model->row_count();
    }) == 1);
}

TEST_CASE("Image")
{
    using namespace sixtyfps;

    // ensure a backend exists, using private api
    private_api::WindowRc wnd;

    Image img;
    {
        auto size = img.size();
        REQUIRE(size.width == 0.);
        REQUIRE(size.height == 0.);
    }
    {
        REQUIRE(!img.path().has_value());
    }

    img = Image::load_from_path(SOURCE_DIR "/../../vscode_extension/extension-logo.png");
    {
        auto size = img.size();
        REQUIRE(size.width == 128.);
        REQUIRE(size.height == 128.);
    }
    {
        auto actual_path = img.path();
        REQUIRE(actual_path.has_value());
        REQUIRE(*actual_path == SOURCE_DIR "/../../vscode_extension/extension-logo.png");
    }
}

TEST_CASE("SharedVector")
{
    using namespace sixtyfps;

    SharedVector<SharedString> vec;
    vec.clear();
    vec.push_back("Hello");
    vec.push_back("World");
    vec.push_back("of");
    vec.push_back("Vectors");

    auto copy = vec;

    REQUIRE(vec.size() == 4);
    auto orig_cap = vec.capacity();
    REQUIRE(orig_cap >= vec.size());
    vec.clear();
    REQUIRE(vec.size() == 0);
    REQUIRE(vec.capacity() == 0); // vec was shared, so start with new empty vector.
    vec.push_back("Welcome back");
    REQUIRE(vec.size() == 1);
    REQUIRE(vec.capacity() >= vec.size());

    REQUIRE(copy.size() == 4);
    REQUIRE(copy.capacity() == orig_cap);
    copy.clear(); // copy is not shared (anymore), retain capacity.
    REQUIRE(copy.capacity() == orig_cap);
}
