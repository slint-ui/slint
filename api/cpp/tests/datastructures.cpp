// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include <ranges>
#include <chrono>
#include <filesystem>
#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>

SCENARIO("SharedString API")
{
    slint::SharedString str;

    REQUIRE(str.empty());
    REQUIRE(str.size() == 0);
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

    SECTION("begin/end")
    {
        str = "Hello";
        REQUIRE(str.begin() + std::string_view(str).size() == str.end());
    }

    SECTION("size")
    {
        str = "Hello";
        REQUIRE(str.size() == 5);
    }

    SECTION("clear")
    {
        str = "Hello";
        str.clear();
        REQUIRE(str.size() == 0);
        REQUIRE(std::string_view(str.data()) == "");
    }

    SECTION("to_lowercase")
    {
        str = "Hello";
        REQUIRE(std::string_view(str.to_lowercase().data()) == "hello");
    }
    SECTION("to_uppercase")
    {
        str = "Hello";
        REQUIRE(std::string_view(str.to_uppercase().data()) == "HELLO");
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

TEST_CASE("Image buffer access")
{
    using namespace slint;

    auto img = Image::load_from_path(SOURCE_DIR "/redpixel.png");

    REQUIRE(!img.to_rgb8().has_value());

    {
        auto rgb = img.to_rgba8();
        REQUIRE(rgb.has_value());
        REQUIRE(rgb->width() == 1);
        REQUIRE(rgb->height() == 1);
        REQUIRE(*rgb->begin() == Rgba8Pixel { 255, 0, 0, 255 });
    }

    {
        auto rgb = img.to_rgba8_premultiplied();
        REQUIRE(rgb.has_value());
        REQUIRE(rgb->width() == 1);
        REQUIRE(rgb->height() == 1);
        REQUIRE(*rgb->begin() == Rgba8Pixel { 255, 0, 0, 255 });
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

TEST_CASE("Slice comparison")
{
    using namespace slint;
    using slint::cbindgen_private::Slice;

    int a[] = { 1, 2, 3 };
    int b[] = { 1, 2, 3 };
    int c[] = { 1, 2, 4 };

    // Compute the results outside of REQUIRE: Catch2's expression decomposition would
    // call the comparison operators from its own namespace where ADL can't find them.
    const bool equal_same = Slice<int> { a, 3 } == Slice<int> { b, 3 };
    const bool notequal_same = Slice<int> { a, 3 } != Slice<int> { b, 3 };
    REQUIRE(equal_same);
    REQUIRE_FALSE(notequal_same);

    const bool equal_diff = Slice<int> { a, 3 } == Slice<int> { c, 3 };
    const bool notequal_diff = Slice<int> { a, 3 } != Slice<int> { c, 3 };
    REQUIRE_FALSE(equal_diff);
    REQUIRE(notequal_diff);

    const bool equal_len = Slice<int> { a, 2 } == Slice<int> { a, 3 };
    const bool notequal_len = Slice<int> { a, 2 } != Slice<int> { a, 3 };
    REQUIRE_FALSE(equal_len);
    REQUIRE(notequal_len);
}

TEST_CASE("StyledText")
{
    auto empty_arguments = std::array<slint::StyledText, 0> {};
    auto text = slint::private_api::parse_markdown(
            "Hello *world*", slint::private_api::make_slice(std::span(empty_arguments)));
    auto text_argument = std::array<slint::StyledText, 1> { text };
    // \u{e541} is MARKDOWN_INTERPOLATION_PLACEHOLDER defined in internal/common/styled_text.rs
    auto text2 = slint::private_api::parse_markdown(
            u8"Text: \uE541", slint::private_api::make_slice(std::span(text_argument)));
}

TEST_CASE("StyledText public API")
{
    using slint::SharedString;
    using slint::StyledText;

    SECTION("from_plain_text")
    {
        auto text = StyledText::from_plain_text("Hello world");
        auto text2 = StyledText::from_plain_text("Hello world");
        REQUIRE(text == text2);

        auto empty = StyledText::from_plain_text("");
        REQUIRE(!(text == empty));
    }

    SECTION("from_markdown success")
    {
        auto result = StyledText::from_markdown("Hello *world*!");
        REQUIRE(result.has_value());

        auto result2 = StyledText::from_markdown("Hello *world*!");
        REQUIRE(result2.has_value());
        REQUIRE(*result == *result2);
    }

    SECTION("from_markdown error")
    {
        auto result = StyledText::from_markdown("# heading");
        REQUIRE(!result.has_value());
    }

    SECTION("from_plain_text vs from_markdown")
    {
        auto plain = StyledText::from_plain_text("plain text");
        auto md = StyledText::from_markdown("plain text");
        REQUIRE(md.has_value());
        REQUIRE(plain == *md);
    }

    SECTION("copy and assign")
    {
        auto original = StyledText::from_plain_text("test");
        StyledText copy(original);
        REQUIRE(copy == original);

        StyledText assigned;
        assigned = original;
        REQUIRE(assigned == original);
    }
}

TEST_CASE("DataTransfer")
{
    using slint::DataTransfer;

    SECTION("Default construction")
    {
        DataTransfer a;
        DataTransfer b;
        REQUIRE(a == b);
        REQUIRE(a.is_empty());
    }

    SECTION("Copy construction")
    {
        DataTransfer a;
        DataTransfer b(a);
        REQUIRE(a == b);
    }

    SECTION("Copy assignment")
    {
        DataTransfer a;
        DataTransfer b;
        b = a;
        REQUIRE(a == b);
    }

    SECTION("Self copy assignment")
    {
        DataTransfer a;
        DataTransfer &ref = a;
        a = ref;
        REQUIRE(a == DataTransfer {});
    }

    SECTION("Move construction")
    {
        DataTransfer a;
        DataTransfer b(std::move(a));
        REQUIRE(b == DataTransfer {});
    }

    SECTION("Move assignment")
    {
        DataTransfer a;
        DataTransfer b;
        b = std::move(a);
        REQUIRE(b == DataTransfer {});
    }

    SECTION("Plain text")
    {
        DataTransfer a;
        REQUIRE(!a.has_plain_text());
        REQUIRE(!a.plain_text().has_value());

        a.set_plain_text(slint::SharedString("hello"));
        REQUIRE(a.has_plain_text());
        REQUIRE(!a.is_empty());
        REQUIRE(a.plain_text() == slint::SharedString("hello"));

        // Overwrite.
        a.set_plain_text(slint::SharedString("world"));
        REQUIRE(a.plain_text() == slint::SharedString("world"));

        // Clones share data, modifying one diverges them.
        DataTransfer b(a);
        REQUIRE(a == b);
        b.set_plain_text(slint::SharedString("other"));
        REQUIRE(a != b);
        REQUIRE(a.plain_text() == slint::SharedString("world"));
        REQUIRE(b.plain_text() == slint::SharedString("other"));
    }

    SECTION("Plain text conversion constructor")
    {
        DataTransfer a { slint::SharedString("hi") };
        REQUIRE(a.has_plain_text());
        REQUIRE(!a.has_image());
        REQUIRE(a.plain_text() == slint::SharedString("hi"));
    }

    SECTION("Image")
    {
        DataTransfer a;
        REQUIRE(!a.has_image());
        REQUIRE(!a.image().has_value());

        slint::Image img(slint::SharedPixelBuffer<slint::Rgb8Pixel>(2, 1));
        a.set_image(img);
        REQUIRE(a.has_image());
        auto fetched = a.image();
        REQUIRE(fetched.has_value());
        REQUIRE(fetched->size().width == 2);
        REQUIRE(fetched->size().height == 1);
    }

    SECTION("Image conversion constructor")
    {
        slint::Image img(slint::SharedPixelBuffer<slint::Rgb8Pixel>(3, 4));
        DataTransfer a { img };
        REQUIRE(a.has_image());
        REQUIRE(!a.has_plain_text());
    }

    SECTION("set_plain_text with empty string clears")
    {
        DataTransfer a;
        a.set_plain_text(slint::SharedString("hello"));
        REQUIRE(a.has_plain_text());
        a.set_plain_text(slint::SharedString(""));
        REQUIRE(!a.has_plain_text());
        REQUIRE(!a.plain_text().has_value());
        REQUIRE(a.is_empty());
    }

    SECTION("set_image with default image clears")
    {
        DataTransfer a;
        a.set_image(slint::Image(slint::SharedPixelBuffer<slint::Rgb8Pixel>(2, 2)));
        REQUIRE(a.has_image());
        a.set_image(slint::Image());
        REQUIRE(!a.has_image());
        REQUIRE(!a.image().has_value());
        REQUIRE(a.is_empty());
    }

    SECTION("Plain text and image coexist")
    {
        DataTransfer a;
        a.set_plain_text(slint::SharedString("text"));
        a.set_image(slint::Image(slint::SharedPixelBuffer<slint::Rgb8Pixel>(1, 1)));
        REQUIRE(a.has_plain_text());
        REQUIRE(a.has_image());
    }

    SECTION("File paths round-trip")
    {
        DataTransfer a;
        REQUIRE(!a.has_file_paths());
        REQUIRE(!a.file_paths().has_value());

        std::filesystem::path paths[] = { "/tmp/plain.txt", u8"/tmp/gr\u00fc\u00dfe.txt" };
        a.set_file_paths(paths);
        REQUIRE(a.has_file_paths());
        REQUIRE(!a.is_empty());
        auto fetched = a.file_paths();
        REQUIRE(fetched.has_value());
        REQUIRE(fetched->size() == 2);
        REQUIRE((*fetched)[0] == paths[0]);
        REQUIRE((*fetched)[1] == paths[1]);

        a.set_file_paths({});
        REQUIRE(!a.has_file_paths());
        REQUIRE(a.is_empty());
    }

#ifdef _WIN32
    SECTION("File paths keep unpaired surrogates")
    {
        // A lone surrogate is representable in a Windows filename; it must
        // survive the conversion to Rust's WTF-8 encoded PathBuf and back.
        std::filesystem::path lone_surrogate(L"C:\\tmp\\lone-\xD800.txt");
        std::filesystem::path paths[] = { lone_surrogate };
        DataTransfer a;
        a.set_file_paths(paths);
        auto fetched = a.file_paths();
        REQUIRE(fetched.has_value());
        REQUIRE((*fetched)[0].native() == lone_surrogate.native());
    }
#else
    SECTION("File paths keep non-UTF-8 bytes")
    {
        std::filesystem::path invalid_utf8(std::string("/tmp/\xFF\xFE-invalid"));
        std::filesystem::path paths[] = { invalid_utf8 };
        DataTransfer a;
        a.set_file_paths(paths);
        auto fetched = a.file_paths();
        REQUIRE(fetched.has_value());
        REQUIRE((*fetched)[0].native() == invalid_utf8.native());
    }
#endif

    SECTION("User data round-trip")
    {
        DataTransfer a;
        REQUIRE(!a.has_user_data());
        REQUIRE(!a.user_data().has_value());

        auto value = std::make_shared<int>(42);
        std::weak_ptr<int> observer = value;
        REQUIRE(observer.use_count() == 1);

        a.set_user_data(value);
        REQUIRE(a.has_user_data());
        REQUIRE(observer.use_count() == 2); // a + the local `value`

        std::any fetched = a.user_data();
        REQUIRE(fetched.has_value());
        auto *fetched_ptr = std::any_cast<std::shared_ptr<int>>(&fetched);
        REQUIRE(fetched_ptr != nullptr);
        REQUIRE(**fetched_ptr == 42);
        REQUIRE(observer.use_count() == 3); // a + value + fetched
    }

    SECTION("User data type mismatch")
    {
        DataTransfer a;
        a.set_user_data(std::make_shared<int>(7));
        REQUIRE(a.has_user_data());
        std::any v = a.user_data();
        REQUIRE(std::any_cast<std::shared_ptr<double>>(&v) == nullptr);
        REQUIRE(std::any_cast<std::shared_ptr<int>>(&v) != nullptr);
    }

    SECTION("User data survives clone")
    {
        DataTransfer a;
        auto value = std::make_shared<int>(99);
        std::weak_ptr<int> observer = value;
        a.set_user_data(value);
        value.reset(); // only `a` and (later) any retrieved shared_ptr keep the value alive
        REQUIRE(observer.use_count() == 1);

        DataTransfer b(a);
        REQUIRE(b.has_user_data());
        REQUIRE(observer.use_count() == 1); // clone of `a` shares the same C++ shared_ptr

        std::any from_a_any = a.user_data();
        std::any from_b_any = b.user_data();
        auto *from_a = std::any_cast<std::shared_ptr<int>>(&from_a_any);
        auto *from_b = std::any_cast<std::shared_ptr<int>>(&from_b_any);
        REQUIRE((from_a && from_b));
        REQUIRE(**from_a == 99);
        REQUIRE(**from_b == 99);
        REQUIRE(from_a->get() == from_b->get());
        REQUIRE(observer.use_count() == 3); // a + b + from_a (== from_b)
    }

    SECTION("Replacing user data drops the old value")
    {
        DataTransfer a;
        auto first = std::make_shared<int>(1);
        std::weak_ptr<int> first_observer = first;
        a.set_user_data(first);
        first.reset();
        REQUIRE(first_observer.use_count() == 1);

        a.set_user_data(std::make_shared<int>(2));
        REQUIRE(first_observer.expired());
        std::any v = a.user_data();
        REQUIRE(**std::any_cast<std::shared_ptr<int>>(&v) == 2);
    }

    SECTION("Clearing user data drops the value")
    {
        DataTransfer a;
        auto value = std::make_shared<int>(5);
        std::weak_ptr<int> observer = value;
        a.set_user_data(value);
        value.reset();
        REQUIRE(observer.use_count() == 1);

        a.clear_user_data();
        REQUIRE(!a.has_user_data());
        REQUIRE(observer.expired());
    }

    SECTION("User data, plain text, and image coexist")
    {
        DataTransfer a;
        a.set_plain_text(slint::SharedString("text"));
        a.set_image(slint::Image(slint::SharedPixelBuffer<slint::Rgb8Pixel>(1, 1)));
        a.set_user_data(std::make_shared<int>(123));
        REQUIRE(a.has_plain_text());
        REQUIRE(a.has_image());
        REQUIRE(a.has_user_data());
        std::any v = a.user_data();
        REQUIRE(**std::any_cast<std::shared_ptr<int>>(&v) == 123);
    }

    SECTION("User data with non-pointer value type")
    {
        DataTransfer a;
        a.set_user_data(42);
        REQUIRE(a.has_user_data());
        std::any v = a.user_data();
        REQUIRE(std::any_cast<int>(&v) != nullptr);
        REQUIRE(*std::any_cast<int>(&v) == 42);
    }
}
