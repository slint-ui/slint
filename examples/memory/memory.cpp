// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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
        int first_visible_index = -1;
        TileData first_visible_tile;
        for (int i = 0; i < tiles_model->row_count(); ++i) {
            auto tile = tiles_model->row_data(i);
            if (!tile.image_visible || tile.solved)
                continue;
            if (first_visible_index == -1) {
                first_visible_index = i;
                first_visible_tile = tile;
                continue;
            }
            bool is_pair_solved = tile == first_visible_tile;
            if (is_pair_solved) {
                first_visible_tile.solved = true;
                tiles_model->set_row_data(first_visible_index, first_visible_tile);
                tile.solved = true;
                tiles_model->set_row_data(i, tile);
                return;
            }
            main_window->set_disable_tiles(true);

            sixtyfps::Timer::single_shot(std::chrono::seconds(1), [=]() mutable {
                main_window->set_disable_tiles(false);
                first_visible_tile.image_visible = false;
                tiles_model->set_row_data(first_visible_index, first_visible_tile);
                tile.image_visible = false;
                tiles_model->set_row_data(i, tile);
            });
        }
    });

    main_window->run();
}
