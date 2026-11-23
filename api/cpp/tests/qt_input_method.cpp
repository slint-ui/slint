// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint-interpreter.h>

#include <QtCore/QCoreApplication>
#include <QtGui/QInputMethodEvent>
#include <QtWidgets/QWidget>

#include <string>

using slint::interpreter::ComponentInstance;
using slint::interpreter::Value;

namespace {

// é: one UTF-16 code unit, two UTF-8 bytes.
const std::string e_acute = "\xc3\xa9";
// 😀: two UTF-16 code units, four UTF-8 bytes.
const std::string grinning_face = "\xf0\x9f\x98\x80";

// A focused TextInput that holds `text`, with the cursor at the UTF-8 offset `cursor`.
slint::ComponentHandle<ComponentInstance> text_input(const std::string &text, int cursor)
{
    slint::interpreter::ComponentCompiler compiler;
    auto definition = compiler.build_from_source(R"(
        export component App inherits Window {
            in-out property <string> text <=> input.text;
            public function place-cursor(offset: int) {
                input.focus();
                input.set-selection-offsets(offset, offset);
            }
            input := TextInput { }
        }
    )",
                                                 "");
    REQUIRE(definition.has_value());
    auto instance = definition->create();
    REQUIRE(instance->set_property("text", slint::SharedString(text)));
    Value args[] = { Value(double(cursor)) };
    REQUIRE(instance->invoke("place-cursor", args).has_value());
    REQUIRE(instance->qwidget() != nullptr);
    return instance;
}

// The text after the input method replaced `length` UTF-16 code units with "x",
// starting `start` code units from the cursor.
std::string replace_with_x(const std::string &text, int cursor, int start, int length)
{
    auto instance = text_input(text, cursor);
    QInputMethodEvent event;
    event.setCommitString("x", start, length);
    QCoreApplication::sendEvent(instance->qwidget(), &event);
    return std::string(*instance->get_property("text")->to_string());
}

}

TEST_CASE("Qt input method queries split the text at the cursor")
{
    const std::string text = "a" + e_acute + grinning_face + "b";
    auto instance = text_input(text, 3);
    auto query = [&](Qt::InputMethodQuery property) {
        return instance->qwidget()->inputMethodQuery(property);
    };

    REQUIRE(query(Qt::ImSurroundingText).toString().toStdString() == text);
    REQUIRE(query(Qt::ImCursorPosition).toInt() == 2);
    REQUIRE(query(Qt::ImTextBeforeCursor).toString().toStdString() == "a" + e_acute);
    REQUIRE(query(Qt::ImTextAfterCursor).toString().toStdString() == grinning_face + "b");
}

TEST_CASE("Qt input method events replace text around the cursor")
{
    SECTION("nothing")
    {
        REQUIRE(replace_with_x("ab", 1, 0, 0) == "axb");
    }
    SECTION("before the cursor at the end of the text")
    {
        REQUIRE(replace_with_x("ab", 2, -1, 1) == "ax");
    }
    SECTION("before the cursor, where UTF-16 and UTF-8 lengths differ")
    {
        REQUIRE(replace_with_x("a" + e_acute + grinning_face + "b", 7, -3, 3) == "axb");
    }
    SECTION("after the cursor, where UTF-16 and UTF-8 lengths differ")
    {
        REQUIRE(replace_with_x(e_acute + e_acute + "b", 0, 0, 2) == "xb");
    }
    SECTION("on both sides of the cursor")
    {
        REQUIRE(replace_with_x("a" + e_acute + "b", 1, -1, 3) == "x");
    }
    SECTION("past both ends of the text")
    {
        REQUIRE(replace_with_x("ab", 1, -5, 9) == "x");
    }
}
