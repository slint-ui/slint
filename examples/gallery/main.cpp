// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "gallery.h"

#ifdef HAVE_GETTEXT
#    include <locale>
#    include <libintl.h>
#endif

int main()
{
#ifdef HAVE_GETTEXT
    bindtextdomain("gallery", SRC_DIR "/lang/");
    std::locale::global(std::locale(""));
#endif

    auto demo = App::create();

    auto row_data = std::make_shared<
            slint::VectorModel<std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>();

    for (int r = 1; r < 101; r++) {

        auto items = std::make_shared<slint::VectorModel<slint::StandardListViewItem>>();

        for (int c = 1; c < 5; c++) {
            slint::SharedString text("item");
            text = text + slint::SharedString::from_number(c) + slint::SharedString(".")
                    + slint::SharedString::from_number(r);
            items->push_back(slint::StandardListViewItem { text });
        }

        row_data->push_back(items);
    }

    demo->global<TableViewPageAdapter>().set_row_data(row_data);

    demo->global<TableViewPageAdapter>().on_filter_sort_model([](auto source_model,
                                                                 slint::SharedString filter,
                                                                 int sort_index,
                                                                 bool sort_ascending) -> auto {
        auto model = source_model;

        if (!filter.empty()) {
            auto l_filter = filter.to_lowercase();
            model = std::make_shared<
                    slint::FilterModel<std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>(
                    source_model,
                    [l_filter](const std::shared_ptr<slint::Model<slint::StandardListViewItem>> e)
                            -> bool {
                        // filter first row
                        std::string text(e->row_data(0).value().text.to_lowercase());

                        return text.find(l_filter) != std::string::npos;
                    });
        }

        if (sort_index >= 0) {
            model = std::make_shared<
                    slint::SortModel<std::shared_ptr<slint::Model<slint::StandardListViewItem>>>>(
                    model, [sort_index, sort_ascending](auto lhs, auto rhs) {
                        auto c_lhs = lhs->row_data(sort_index);
                        auto c_rhs = rhs->row_data(sort_index);

                        return sort_ascending ? c_lhs->text < c_rhs->text
                                              : c_rhs->text < c_lhs->text;
                    });
        }

        return model;
    });

    demo->run();
}
