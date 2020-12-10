/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#include "memory.h"
#include <random>

int main()
{
    auto main_window = MainWindow::create();
    auto old_tiles = main_window->get_memory_tiles();
    std::vector<TileData> new_tiles;
    new_tiles.reserve(old_tiles->row_count() * 2);
    for (int i = 0; i < old_tiles->row_count(); ++i) {
        new_tiles.push_back(old_tiles->row_data(i));
        new_tiles.push_back(old_tiles->row_data(i));
    }
    std::default_random_engine rng{};
    std::shuffle(std::begin(new_tiles), std::end(new_tiles), rng);
    auto tiles_model = std::make_shared<sixtyfps::VectorModel<TileData>>(new_tiles);
    main_window->set_memory_tiles(tiles_model);

    main_window->on_check_if_pair_solved([main_window_weak = sixtyfps::ComponentWeakHandle(main_window)] {
        auto main_window = *main_window_weak.lock();
        auto tiles_model = main_window->get_memory_tiles();
        int index1 = -1;
        TileData tile1;
        for (int i = 0; i < tiles_model->row_count(); ++i) {
            auto tile = tiles_model->row_data(i);
            if (!tile.image_visible || tile.solved)
                continue;
            if (index1 == -1) {
                index1 = i;
                tile1 = tile;
                continue;
            }
            bool is_pair_solved = tile == tile1;
            if (is_pair_solved) {
                tile1.solved = true;
                tiles_model->set_row_data(index1, tile1);
                tile.solved = true;
                tiles_model->set_row_data(i, tile);
                return;
            }
            main_window->set_disable_tiles(true);

            sixtyfps::Timer::single_shot(std::chrono::seconds(1), [=]() mutable {
                main_window->set_disable_tiles(false);
                tile1.image_visible = false;
                tiles_model->set_row_data(index1, tile1);
                tile.image_visible = false;
                tiles_model->set_row_data(i, tile);
            });
        }
    });

    main_window->run();
}
