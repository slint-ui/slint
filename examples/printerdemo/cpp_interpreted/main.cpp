// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include <slint-interpreter.h>

#include <ctime>

using slint::interpreter::Value;

struct InkLevelModel : slint::Model<Value>
{
    size_t row_count() const override { return m_data.size(); }
    std::optional<Value> row_data(size_t i) const override
    {
        if (i < m_data.size())
            return { m_data[i] };
        return {};
    }

private:
    static Value make_inklevel_value(slint::Color color, float level)
    {
        slint::interpreter::Struct s;
        s.set_field("color", Value(color));
        s.set_field("level", level);
        return s;
    }

    std::vector<Value> m_data = {
        make_inklevel_value(slint::Color::from_rgb_uint8(255, 255, 0), 0.9),
        make_inklevel_value(slint::Color::from_rgb_uint8(255, 0, 255), 0.8),
        make_inklevel_value(slint::Color::from_rgb_uint8(0, 255, 255), 0.5),
        make_inklevel_value(slint::Color::from_rgb_uint8(0, 0, 0), 0.1),
    };
};

int main()
{
    slint::interpreter::ComponentCompiler compiler;
    auto definition = compiler.build_from_path(SOURCE_DIR "/../ui/printerdemo.slint");

    for (auto diagnostic : compiler.diagnostics()) {
        std::cerr << (diagnostic.level == slint::interpreter::DiagnosticLevel::Warning ? "warning: "
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
    std::shared_ptr<slint::Model<Value>> ink_levels = std::make_shared<InkLevelModel>();
    if (!instance->set_property("ink_levels", ink_levels)) {
        std::cerr << "Could not set property ink_levels" << std::endl;
        return EXIT_FAILURE;
    }

    auto printer_queue = std::make_shared<slint::VectorModel<Value>>();

    slint::SharedVector<Value> default_queue =
            *instance->get_global_property("PrinterQueue", "printer_queue")->to_array();
    for (const auto &default_item : default_queue)
        printer_queue->push_back(default_item);

    instance->set_global_property("PrinterQueue", "printer_queue", Value(printer_queue));

    instance->set_global_callback("PrinterQueue", "start_job", [=](auto args) {
        std::time_t now = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
        char time_buf[100] = { 0 };
        std::strftime(time_buf, sizeof(time_buf), "%H:%M:%S %d/%m/%Y", std::localtime(&now));

        slint::interpreter::Struct item { { "status", Value(slint::SharedString("WAITING...")) },
                                          { "progress", Value(0.) },
                                          { "title", args[0] },
                                          { "owner", slint::SharedString("joe@example.com") },
                                          { "pages", Value(1.) },
                                          { "size", slint::SharedString("100kB") },
                                          { "submission_date", slint::SharedString(time_buf) } };
        printer_queue->push_back(item);
        return Value();
    });

    instance->set_global_callback("PrinterQueue", "cancel_job", [=](auto args) {
        auto index = *args[0].to_number();
        printer_queue->erase(int(index));
        return Value();
    });

    slint::Timer printer_queue_progress_timer(std::chrono::seconds(1), [=]() {
        if (printer_queue->row_count() > 0) {
            auto top_item = *(*printer_queue->row_data(0)).to_struct();
            auto progress = *top_item.get_field("progress")->to_number() + 1.;
            top_item.set_field("progress", progress);
            top_item.set_field("status", slint::SharedString("PRINTING"));
            if (progress > 100) {
                printer_queue->erase(0);
            } else {
                printer_queue->set_row_data(0, top_item);
            }
        }
    });

    instance->set_callback("quit", [](auto) {
        std::exit(0);
        return Value();
    });
    instance->run();
}
