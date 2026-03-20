// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>

#include <slint-interpreter.h>

static int destructor_call_count = 0;

struct TestTranslator : public slint::Translator
{
    ~TestTranslator() { ++destructor_call_count; }

    slint::SharedString translate(std::string_view string, std::string_view context) const override
    {
        return slint::SharedString("translate(string=") + string + ", context=" + context + ")";
    }

    slint::SharedString ntranslate(uint64_t n, std::string_view singular, std::string_view plural,
                                   std::string_view context) const override
    {
        return slint::SharedString("ntranslate(n=") + slint::SharedString::from_number(n)
                + ", singular=" + singular + ", plural=" + plural + ", context=" + context + ")";
    }
};

TEST_CASE("Translate Properties")
{
    using namespace slint::interpreter;
    using namespace slint;

    ComponentCompiler compiler;
    auto comp_def = compiler.build_from_source(R"(
        export App := Window {
            out property <string> singular: @tr("singular");
            out property <string> plural: @tr("singular" | "plural-{n}" % 42);
        }
    )",
                                               "");
    REQUIRE(comp_def.has_value());
    auto instance = comp_def->create();
    REQUIRE(instance->get_property("singular").has_value());
    REQUIRE(instance->get_property("plural").has_value());

    // Check before registering a translator, should return the original string.
    REQUIRE(*instance->get_property("singular")->to_string() == "singular");
    REQUIRE(*instance->get_property("plural")->to_string() == "plural-42");

    // Registering a translator should automatically update the properties.
    slint::set_translator(std::make_unique<TestTranslator>());
    REQUIRE(*instance->get_property("singular")->to_string()
            == "translate(string=singular, context=App)");
    REQUIRE(*instance->get_property("plural")->to_string()
            == "ntranslate(n=42, singular=singular, plural=plural-42, context=App)");

    // Unregistering the translator should restore the untranslated strings.
    slint::set_translator(nullptr);
    REQUIRE(*instance->get_property("singular")->to_string() == "singular");
    REQUIRE(*instance->get_property("plural")->to_string() == "plural-42");
}

TEST_CASE("Set/Unset Translator")
{
    destructor_call_count = 0;

    // Set nullptr should be allowed but has no effect.
    slint::set_translator(nullptr);

    // Registering a translator should not call the destructor.
    slint::set_translator(std::make_unique<TestTranslator>());
    REQUIRE(destructor_call_count == 0);

    // Registering another translator should destroy the first translator.
    slint::set_translator(std::make_unique<TestTranslator>());
    REQUIRE(destructor_call_count == 1);

    // Unregistering the translator should destroy the second translator.
    slint::set_translator(nullptr);
    REQUIRE(destructor_call_count == 2);
}
