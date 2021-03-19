/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#include <sixtyfps_interpreter.h>


using sixtyfps::interpreter::Value;


struct InkLevelModel : sixtyfps::Model<Value>
{
    int row_count() const override { return m_data.size(); }
    Value row_data(int i) const override { return m_data[i]; }
private:
    static Value make_inklevel_value(sixtyfps::Color color, float level) {
        sixtyfps::interpreter::Struct s;
        s.set_field("color", Value(color));
        s.set_field("level", level);
        return s;
    }

    std::vector<Value> m_data = {
        make_inklevel_value( sixtyfps::Color::from_rgb_uint8(255, 255, 0), 0.9 ),
        make_inklevel_value( sixtyfps::Color::from_rgb_uint8(255, 0, 255), 0.8 ),
        make_inklevel_value( sixtyfps::Color::from_rgb_uint8(0, 255, 255), 0.5 ),
        make_inklevel_value( sixtyfps::Color::from_rgb_uint8(0, 0, 0), 0.1 ),
    };
};

int main()
{
    if (auto error = sixtyfps::register_font_from_path(FONTS_DIR "/NotoSans-Regular.ttf")) {
        fprintf(stderr, "Error registering Noto Sans Regular font: %s\n", error->data());
    }
    if (auto error = sixtyfps::register_font_from_path(FONTS_DIR "/NotoSans-Bold.ttf")) {
        fprintf(stderr, "Error registering Noto Sans Bold font: %s\n", error->data());
    }

    sixtyfps::interpreter::ComponentCompiler compiler;
    auto definition = compiler.build_from_path(__FILE__ "/../../ui/printerdemo.60");
    // FIXME: show diagnostics
    if (!definition) {
        std::cerr << "compilation failure" << std::endl;
        return EXIT_FAILURE;
    }
    auto instance = definition->create();
    std::shared_ptr<sixtyfps::Model<Value>> ink_levels = std::make_shared<InkLevelModel>();
    if (!instance->set_property("ink_levels", ink_levels)) {
        std::cerr << "Could not set property ink_levels" << std::endl;
        return EXIT_FAILURE;
    }
    instance->set_callback("quit", [](auto) { std::exit(0); return Value(); });
    instance->run();
}
