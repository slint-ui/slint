/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#include <sixtyfps_interpreter.h>

#include <ctime>

using sixtyfps::interpreter::Value;

int main()
{
    sixtyfps::interpreter::ComponentCompiler compiler;
    auto definition = compiler.build_from_path(SOURCE_DIR "/main.60");

    for (auto diagnostic : compiler.diagnostics()) {
        std::cerr << (diagnostic.level == sixtyfps::interpreter::DiagnosticLevel::Warning
                              ? "warning: "
                              : "error: ")
                  << diagnostic.message << std::endl;
        std::cerr << "location: " << diagnostic.source_file;
        if (diagnostic.line > 0)
            std::cerr << ":" << diagnostic.line;
        if (diagnostic.column > 0)
            std::cerr << ":" << diagnostic.column;
        std::cerr << std::endl;
    }

    if (!definition) {
        std::cerr << "compilation failure!" << std::endl;
        return EXIT_FAILURE;
    }
    auto instance = definition->create();

    instance->run();
}
