// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include "gallery.h"

int main()
{
    auto demo = App::create();

    auto row_data = std::make_shared<
            slint::VectorModel<std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>();

    for (int r = 1; r < 101; r++) {

        auto items = std::make_shared<slint::VectorModel<slint::StandardListViewItem>>();

        for (int c = 1; c < 5; c++) {
            slint::SharedString text("item");
            text = text + slint::SharedString::from_number(c) + slint::SharedString(".")
                    + slint::SharedString::from_number(r);
            items->push_back(slint::StandardListViewItem { text, c == 1 });
        }

        row_data->push_back(items);
    }

    demo->global<TableViewPageAdapter>().set_row_data(row_data);

    demo->global<TableViewPageAdapter>().on_sort_ascending([row_data,
                                                            demo = slint::ComponentWeakHandle(
                                                                    demo)](int index) {
        auto demo_lock = demo.lock();
        (*demo_lock)
                ->global<TableViewPageAdapter>()
                .set_row_data(std::make_shared<slint::SortModel<
                                      std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>(
                        row_data, [index](auto lhs, auto rhs) {
                            auto c_lhs = lhs->row_data(index);
                            auto c_rhs = rhs->row_data(index);

                            return c_lhs->text < c_rhs->text;
                        }));
    });

    demo->global<TableViewPageAdapter>().on_sort_descending(
            [row_data, demo = slint::ComponentWeakHandle(demo)](int index) {
                auto demo_lock = demo.lock();
                (*demo_lock)->global<TableViewPageAdapter>().set_row_data(row_data);

        (*demo_lock)
                ->global<TableViewPageAdapter>()
                .set_row_data(std::make_shared<slint::SortModel<
                                      std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>(
                        (*demo_lock)->global<TableViewPageAdapter>().get_row_data(),
                        [index](auto lhs, auto rhs) {
                            auto c_lhs = lhs->row_data(index);
                            auto c_rhs = rhs->row_data(index);

                            return c_rhs->text < c_lhs->text;
                        }));
    });

    demo->run();
}
