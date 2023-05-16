// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

#include "printerdemo.h"

struct InkLevelModel : slint::Model<InkLevel>
{
    size_t row_count() const override { return m_data.size(); }
    std::optional<InkLevel> row_data(size_t i) const override
    {
        if (i < row_count())
            return { m_data[i] };
        return {};
    }

    std::vector<InkLevel> m_data = { { slint::Color::from_rgb_uint8(255, 255, 0), 0.9 },
                                     { slint::Color::from_rgb_uint8(0, 255, 255), 0.5 },
                                     { slint::Color::from_rgb_uint8(255, 0, 255), 0.8 },
                                     { slint::Color::from_rgb_uint8(0, 0, 0), 0.1 } };
};

int main()
{
    auto printer_demo = MainWindow::create();
    printer_demo->set_ink_levels(std::make_shared<InkLevelModel>());
    printer_demo->on_quit([] { std::exit(0); });

    printer_demo->on_fax_number_erase([printer_demo = slint::ComponentWeakHandle(printer_demo)] {
        std::string fax_number { (*printer_demo.lock())->get_fax_number() };
        fax_number.pop_back();
        (*printer_demo.lock())->set_fax_number(fax_number.data());
    });

    printer_demo->on_fax_send([printer_demo = slint::ComponentWeakHandle(printer_demo)] {
        std::cout << "Sending a fax to " << (*printer_demo.lock())->get_fax_number() << std::endl;
        (*printer_demo.lock())->set_fax_number({});
    });

    printer_demo->run();
}
