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
#include <fmt/chrono.h>

using sixtyfps::interpreter::Value;

struct PropertyDeclaration
{
    std::string name;
    std::string type_name;
};

class Widget
{
public:
    virtual ~Widget() { }
    virtual std::string type_name() const = 0;
    virtual std::vector<PropertyDeclaration> properties() const = 0;

    void set_property(std::string_view name, const sixtyfps::interpreter::Value &value)
    {
        if (m_ui)
            (*m_ui)->set_property(qualified_property_name(name), value);
    }

    std::optional<sixtyfps::interpreter::Value> property(std::string_view name) const
    {
        if (m_ui)
            return (*m_ui)->get_property(qualified_property_name(name));
        return {};
    }

    void connect_ui(const sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance> &ui,
                    std::string_view properties_prefix)
    {
        m_ui = ui;
        m_properties_prefix = properties_prefix;
    }

private:
    std::string qualified_property_name(std::string_view name) const
    {
        std::string qname(m_properties_prefix);
        qname += name;
        return qname;
    }

    std::optional<sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance>> m_ui;
    std::string m_properties_prefix;
};

using WidgetPtr = std::shared_ptr<Widget>;

class PlaceholderWidget : public Widget
{
public:
    PlaceholderWidget(std::string_view type_name) : m_type_name(type_name) { }

    std::string type_name() const override { return m_type_name; }
    std::vector<PropertyDeclaration> properties() const override { return {}; }

private:
    std::string m_type_name;
};

class ClockWidget : public Widget
{
public:
    ClockWidget();
    std::string type_name() const override { return "Clock"; }
    std::vector<PropertyDeclaration> properties() const override
    {
        return { PropertyDeclaration { "time", "string" } };
    }

private:
    sixtyfps::Timer clock_update_timer;
};

ClockWidget::ClockWidget()
    : clock_update_timer(std::chrono::seconds(1), [=]() {
          std::string current_time = fmt::format("{:%H:%M:%S}", fmt::localtime(std::time(nullptr)));
          set_property("time", sixtyfps::SharedString(current_time));
      })
{
}

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
    void add_grid_widget(WidgetPtr widget, const WidgetLocation &location);
    void add_top_bar_widget(WidgetPtr widget);

    std::optional<sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance>>
    build(sixtyfps::interpreter::ComponentCompiler &compiler) const;

private:
    std::string register_widget(WidgetPtr widget);

    std::unordered_set<std::string> widgets_used = { "TopBar", "MenuBar" };
    std::string top_bar;
    std::string main_grid;

    std::vector<std::pair<std::string, WidgetPtr>> widgets;
};

void DashboardBuilder::add_grid_widget(WidgetPtr widget, const WidgetLocation &location)
{
    auto widget_name = register_widget(widget);

    main_grid.append(fmt::format(
            R"60(
        {0} := {1} {{
            {2}
        }}
    )60",
            widget_name, widget->type_name(), location.location_bindings()));
}

void DashboardBuilder::add_top_bar_widget(WidgetPtr widget)
{
    auto widget_name = register_widget(widget);

    top_bar.append(fmt::format(
            R"60(
        {0} := {1} {{            
        }}
    )60",
            widget_name, widget->type_name()));
}

std::string DashboardBuilder::register_widget(WidgetPtr widget)
{
    auto widget_type_name = widget->type_name();
    widgets_used.insert(widget_type_name);

    auto widget_id = widgets.size();
    auto widget_name = fmt::format("widget_{}", widget_id);
    widgets.push_back({ widget_name, widget });
    return widget_name;
}

std::optional<sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance>>
DashboardBuilder::build(sixtyfps::interpreter::ComponentCompiler &compiler) const
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

    std::string exposed_properties;

    for (const auto &entry : widgets) {
        auto [widget_name, widget_ptr] = entry;

        std::string properties_prefix = widget_name;
        properties_prefix += "__";

        for (const auto &property : widget_ptr->properties()) {
            std::string qualified_prop_name = properties_prefix + property.name;
            exposed_properties +=
                    fmt::format("property <{0}> {1} <=> {2}.{3};\n", property.type_name,
                                qualified_prop_name, widget_name, property.name);
        }
    }

    auto source_code = fmt::format(
            R"60(

{0}

MainContent := VerticalLayout {{
    spacing: 24px;
    TopBar {{
        @children
    }}

    GridLayout {{
        padding-left: 19px;
        padding-top: 0px;
        padding-right: 17px;
        padding-bottom: 24px;

        {2}
    }}
}}

MainWindow := Window {{
    width: 1024px;
    height: 600px;
    title: "IOT dashboard";

    {3}

    HorizontalLayout {{
        padding: 0; spacing: 0;
        MenuBar {{
        }}
        MainContent {{
            {1}
        }}
    }}
}}
)60",
            widget_imports, top_bar, main_grid, exposed_properties);

    auto definition = compiler.build_from_source(source_code, SOURCE_DIR);

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
        std::cerr << "generated source:" << std::endl << source_code << std::endl;
        return {};
    }

    // std::cerr << source_code << std::endl;

    auto ui = definition->create();

    for (const auto &entry : widgets) {
        auto [widget_name, widget_ptr] = entry;

        std::string properties_prefix = widget_name;
        properties_prefix += "__";

        widget_ptr->connect_ui(ui, properties_prefix);
    }

    return ui;
}

int main()
{
    DashboardBuilder builder;
    builder.add_top_bar_widget(std::make_shared<ClockWidget>());
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("Usage"), { 0, 0, 2 });
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("IndoorTemperature"), { 0, 1 });
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("Humidity"), { 1, 1 });
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("MyDevices"), { 0, 2, 2 });
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("UsageDiagram"), { 2, 0, {}, 2 });
    builder.add_grid_widget(std::make_shared<PlaceholderWidget>("LightIntensity"), { 2, 2 });

    sixtyfps::interpreter::ComponentCompiler compiler;
    compiler.set_include_paths({ SOURCE_DIR });
    auto dashboard = builder.build(compiler);

    if (!dashboard) {
        return EXIT_FAILURE;
    }

    (*dashboard)->run();
}
