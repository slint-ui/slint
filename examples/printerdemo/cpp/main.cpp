/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#include "printerdemo.h"

struct InkLevelModel : sixtyfps::Model {
    int count() const override { return m_data.size(); }
    const void *get(int i) const override { return &m_data[i]; }

    /// FIXME: Ideally it should be a better type in the generated code
    using InkData = std::tuple<sixtyfps::Color, float>;
    std::vector<InkData> m_data = {
        { sixtyfps::Color(0xffffff00), 0.9 },
        { sixtyfps::Color(0xff00ffff), 0.5 },
        { sixtyfps::Color(0xffff00ff), 0.8 },
        { sixtyfps::Color(0xff000000), 0.1 }};
};

int main()
{
    static MainWindow printer_demo;
    printer_demo.set_ink_levels(std::make_shared<InkLevelModel>());
    printer_demo.run();
}
