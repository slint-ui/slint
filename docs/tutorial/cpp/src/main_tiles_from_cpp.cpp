// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// main.cpp

#include "memory_tiles_from_cpp.h" // generated header from memory_tiles_from_cpp.slint
// ANCHOR: main
// ...

#include <random> // Added

int main()
{
    auto main_window = MainWindow::create();
    auto old_tiles = main_window->get_memory_tiles();
    std::vector<TileData> new_tiles;
    new_tiles.reserve(old_tiles->row_count() * 2);
    for (int i = 0; i < old_tiles->row_count(); ++i) {
        new_tiles.push_back(*old_tiles->row_data(i));
        new_tiles.push_back(*old_tiles->row_data(i));
    }
    std::default_random_engine rng {};
    std::shuffle(new_tiles.begin(), new_tiles.end(), rng);
    auto tiles_model = std::make_shared<slint::VectorModel<TileData>>(new_tiles);
    main_window->set_memory_tiles(tiles_model);

    main_window->run();
}
// ANCHOR_END: main
