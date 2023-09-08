// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <slint.h>
#include <slint_image.h>

SCENARIO("SharedString API")
{
    slint::SharedString str;

    REQUIRE(str.empty());
    REQUIRE(str == "");
    REQUIRE(std::string_view(str.data()) == ""); // this test null termination of data()

    SECTION("Construct from string_view")
    {
        std::string foo("Foo");
        std::string_view foo_view(foo);
        str = foo_view;
        REQUIRE(str == "Foo");
        REQUIRE(std::string_view(str.data()) == "Foo");
    }

    SECTION("Construct from char*")
    {
        str = "Bar";
        REQUIRE(str == "Bar");
    }

    SECTION("concatenate")
    {
        str = "Hello";
        str += " ";
        str += slint::SharedString("🦊") + slint::SharedString("!");
        REQUIRE(str == "Hello 🦊!");
        REQUIRE(std::string_view(str.data()) == "Hello 🦊!");
    }
}

TEST_CASE("Basic SharedVector API", "[vector]")
{
    slint::SharedVector<int> vec;
    REQUIRE(vec.empty());

    SECTION("Initializer list")
    {
        slint::SharedVector<int> vec({ 1, 4, 10 });
        REQUIRE(vec.size() == 3);
        REQUIRE(vec[0] == 1);
        REQUIRE(vec[1] == 4);
        REQUIRE(vec[2] == 10);
    }
}

TEST_CASE("Property Tracker")
{
    using namespace slint::private_api;
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
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>();

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

TEST_CASE("Track model row data changes")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    PropertyTracker tracker;

    REQUIRE(tracker.evaluate([&]() {
        model->track_row_data_changes(1);
        return model->row_data(1);
    }) == 1);
    REQUIRE(!tracker.is_dirty());

    model->set_row_data(2, 42);
    REQUIRE(!tracker.is_dirty());
    model->set_row_data(1, 100);
    REQUIRE(tracker.is_dirty());

    REQUIRE(tracker.evaluate([&]() {
        model->track_row_data_changes(1);
        return model->row_data(1);
    }) == 100);
    REQUIRE(!tracker.is_dirty());

    // Any changes to rows (even if after tracked rows) for now also marks watched rows as dirty, to
    // keep the logic simple.
    model->push_back(200);
    REQUIRE(tracker.is_dirty());

    REQUIRE(tracker.evaluate([&]() {
        model->track_row_data_changes(1);
        return model->row_data(1);
    }) == 100);
    REQUIRE(!tracker.is_dirty());

    model->insert(0, 255);
    REQUIRE(tracker.is_dirty());
}

TEST_CASE("Image")
{
    using namespace slint;

    Image img;
    {
        auto size = img.size();
        REQUIRE(size.width == 0.);
        REQUIRE(size.height == 0.);
    }
    {
        REQUIRE(!img.path().has_value());
    }

#ifndef SLINT_FEATURE_FREESTANDING
    img = Image::load_from_path(SOURCE_DIR "/../../../logo/slint-logo-square-light-128x128.png");
    {
        auto size = img.size();
        REQUIRE(size.width == 128.);
        REQUIRE(size.height == 128.);
    }
    {
        auto actual_path = img.path();
        REQUIRE(actual_path.has_value());
        REQUIRE(*actual_path == SOURCE_DIR "/../../../logo/slint-logo-square-light-128x128.png");
    }
#endif

    img = Image(SharedPixelBuffer<Rgba8Pixel> {});
    {
        auto size = img.size();
        REQUIRE(size.width == 0);
        REQUIRE(size.height == 0);
        REQUIRE(!img.path().has_value());
    }
    auto red = Rgb8Pixel { 0xff, 0, 0 };
    auto blu = Rgb8Pixel { 0, 0, 0xff };
    Rgb8Pixel some_data[] = { red, red, blu, red, blu, blu };
    img = Image(SharedPixelBuffer<Rgb8Pixel>(3, 2, some_data));
    {
        auto size = img.size();
        REQUIRE(size.width == 3);
        REQUIRE(size.height == 2);
        REQUIRE(!img.path().has_value());
    }
}

TEST_CASE("SharedVector")
{
    using namespace slint;

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

    SharedVector<SharedString> vec2 { "Hello", "World", "of", "Vectors" };
    REQUIRE(copy == vec2);
    REQUIRE(copy != vec);

    copy.clear(); // copy is not shared (anymore), retain capacity.
    REQUIRE(copy.capacity() == orig_cap);

    SharedVector<SharedString> vec3(2, "Welcome back");
    REQUIRE(vec3.size() == 2);
    REQUIRE(vec3[1] == "Welcome back");
    REQUIRE(vec3 != vec);

    vec.push_back("Welcome back");
    REQUIRE(vec3 == vec);

    SharedVector<int> vec4(5);
    REQUIRE(vec4.size() == 5);
    REQUIRE(vec4[3] == 0);

    std::vector<SharedString> std_v(vec2.begin(), vec2.end());
    SharedVector<SharedString> vec6(std_v.begin(), std_v.end());
    REQUIRE(vec6 == vec2);
}
