/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include <sixtyfps_interpreter.h>

#include <optional>
#include <string_view>
#include <vector>
#include <unordered_set>

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

    void set_property(std::string_view name, const sixtyfps::interpreter::Value &value);

    std::optional<sixtyfps::interpreter::Value> property(std::string_view name) const;

    void connect_ui(const sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance> &ui,
                    std::string_view properties_prefix);

private:
    std::string qualified_property_name(std::string_view name) const;

    std::optional<sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance>> m_ui;
    std::string m_properties_prefix;
};

using WidgetPtr = std::shared_ptr<Widget>;

struct WidgetLocation
{
    int row = 0;
    int column = 0;
    std::optional<int> row_span;
    std::optional<int> col_span;

    std::string location_bindings() const;
};

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
