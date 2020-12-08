/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use sixtyfps::{Model, ModelHandle, Timer, VecModel};
use std::rc::Rc;
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::include_modules!();

fn shuffle(tiles: &mut Vec<TileData>) {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    tiles.shuffle(&mut rng);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();

    let mut tiles: Vec<TileData> = main_window.get_tile_options().iter().collect();
    tiles.extend(tiles.clone());

    shuffle(&mut tiles);

    let tiles_model = Rc::new(VecModel::from(tiles));

    main_window.set_memory_tiles(ModelHandle::new(tiles_model.clone()));

    let main_window_weak = main_window.as_weak();

    main_window.on_check_if_pair_solved(move || {
        let mut flipped_tiles = Vec::new();

        for (index, tile) in tiles_model.iter().enumerate() {
            if tile.image_visible && !tile.solved {
                let index_tile_pair = (index, tile.clone());
                flipped_tiles.push(index_tile_pair);
            }
        }

        if flipped_tiles.len() == 2 {
            main_window_weak.unwrap().set_disable_tiles(true);

            let is_pair_solved = flipped_tiles[0].1 == flipped_tiles[1].1;

            let tiles_model = tiles_model.clone();
            let main_window_weak = main_window_weak.clone();

            Timer::single_shot(Duration::from_secs(1), move || {
                main_window_weak.unwrap().set_disable_tiles(false);

                for (index, mut tile) in flipped_tiles.into_iter() {
                    tile.solved = is_pair_solved;
                    tile.image_visible = is_pair_solved;
                    tiles_model.set_row_data(index, tile);
                }
            })
        }
    });

    main_window.run();
}
