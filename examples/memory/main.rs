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

    let tile_options = main_window.get_tile_options();
    let mut tiles: Vec<TileData> = tile_options.iter().chain(tile_options.iter()).collect();
    shuffle(&mut tiles);
    let tiles_model = Rc::new(VecModel::from(tiles));

    main_window.set_memory_tiles(ModelHandle::new(tiles_model.clone()));

    let main_window_weak = main_window.as_weak();

    main_window.on_check_if_pair_solved(move || {
        let mut flipped_tiles =
            tiles_model.iter().enumerate().filter(|(_, tile)| tile.image_visible && !tile.solved);

        if let (Some((t1_idx, mut t1)), Some((t2_idx, mut t2))) =
            (flipped_tiles.next(), flipped_tiles.next())
        {
            let is_pair_solved = t1 == t2;
            if is_pair_solved {
                t1.solved = true;
                tiles_model.set_row_data(t1_idx, t1.clone());
                t2.solved = true;
                tiles_model.set_row_data(t2_idx, t2.clone());
            } else {
                main_window_weak.unwrap().set_disable_tiles(true);
                let main_window_weak = main_window_weak.clone();
                let tiles_model = tiles_model.clone();
                Timer::single_shot(Duration::from_secs(1), move || {
                    main_window_weak.unwrap().set_disable_tiles(false);
                    t1.image_visible = false;
                    tiles_model.set_row_data(t1_idx, t1);
                    t2.image_visible = false;
                    tiles_model.set_row_data(t2_idx, t2);
                })
            }
        }
    });

    main_window.run();
}
