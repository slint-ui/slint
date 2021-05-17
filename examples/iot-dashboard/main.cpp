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
#include <optional>
#include <string_view>
#include <vector>
#include <unordered_set>
#include <fmt/core.h>

using sixtyfps::interpreter::Value;

struct WidgetLocation
{
    int row = 0;
    int column = 0;
    std::optional<int> row_span;
    std::optional<int> col_span;

    std::string location_bindings() const;
};

std::string WidgetLocation::location_bindings() const
{
    auto maybe_binding = [](std::string_view name, const auto &opt_value) -> std::string {
        if (opt_value.has_value()) {
            return fmt::format("{}: {};", name, opt_value.value());
        } else {
            return "";
        }
    };

    return fmt::format(
            R"60(
            row: {};
            col: {};
            {}
            {}
    )60",
            row, column, maybe_binding("rowspan", row_span), maybe_binding("colspan", col_span));
}

struct DashboardBuilder
{
    void add_widget(std::string_view widget_name, const WidgetLocation &location);

    std::string build() const;

private:
    std::unordered_set<std::string> widgets_used = { "TopBar", "MenuBar" };
    std::string main_grid;
};

void DashboardBuilder::add_widget(std::string_view widget_name, const WidgetLocation &location)
{
    widgets_used.insert(std::string(widget_name));

    main_grid.append(fmt::format(
            R"60(
        {} {{
            {}
        }}
    )60",
            widget_name, location.location_bindings()));
}

std::string DashboardBuilder::build() const
{
    std::string widget_imports;

    for (const auto &widget : widgets_used) {
        if (widget_imports.size() > 0) {
            widget_imports.append(", ");
        }
        widget_imports.append(widget);
    }

    if (widget_imports.size() > 0) {
        widget_imports = fmt::format("import {{ {} }} from \"iot-dashboard.60\";", widget_imports);
    }

    return fmt::format(
            R"60(

{}

MainContent := VerticalLayout {{
    spacing: 24px;
    TopBar {{ }}

    GridLayout {{
        padding-left: 19px;
        padding-top: 0px;
        padding-right: 17px;
        padding-bottom: 24px;

        {}
    }}
}}

MainWindow := Window {{
    width: 1024px;
    height: 600px;
    title: "IOT dashboard";
    HorizontalLayout {{
        padding: 0; spacing: 0;
        MenuBar {{}}
        MainContent {{}}
    }}
}}
)60",
            widget_imports, main_grid);
}

int main()
{
    sixtyfps::interpreter::ComponentCompiler compiler;

    DashboardBuilder builder;
    builder.add_widget("Usage", { 0, 0, 2 });
    builder.add_widget("IndoorTemperature", { 0, 1 });
    builder.add_widget("Humidity", { 1, 1 });
    builder.add_widget("MyDevices", { 0, 2, 2 });
    builder.add_widget("UsageDiagram", { 2, 0, {}, 2 });
    builder.add_widget("LightIntensity", { 2, 2 });

    auto generated_source = builder.build();

    compiler.set_include_paths({ SOURCE_DIR });
    auto definition = compiler.build_from_source(generated_source, SOURCE_DIR);

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
        std::cerr << "generated source:" << std::endl << generated_source << std::endl;
        return EXIT_FAILURE;
    }
    auto instance = definition->create();

    instance->run();
}
