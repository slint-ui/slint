/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#include "dashboard.h"
#include <sixtyfps_interpreter.h>

#include <fmt/core.h>
#include <fmt/chrono.h>

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
    void update_clock();

    sixtyfps::Timer clock_update_timer;
};

ClockWidget::ClockWidget() : clock_update_timer(std::chrono::seconds(1), [=]() { update_clock() })
{
}

void ClockWidget::update_clock()
{
    std::string current_time = fmt::format("{:%H:%M:%S}", fmt::localtime(std::time(nullptr)));
    set_property("time", sixtyfps::SharedString(current_time));
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
