/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
fn main() {
    use sixtyfps::Model;

    let main_window = MainWindow::new();

    // Fetch the tiles from the model
    let mut tiles: Vec<TileData> = main_window.get_memory_tiles().iter().collect();
    // Duplicate them to ensure that we have pairs
    tiles.extend(tiles.clone());

    // Randomly mix the tiles
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    tiles.shuffle(&mut rng);

    // ANCHOR: game_logic
    // Assign the shuffled Vec to the model property
    let tiles_model = std::rc::Rc::new(sixtyfps::VecModel::from(tiles));
    main_window.set_memory_tiles(sixtyfps::ModelHandle::new(tiles_model.clone()));

    let main_window_weak = main_window.as_weak();
    main_window.on_check_if_pair_solved(move || {
        let mut flipped_tiles = tiles_model.iter().enumerate().filter(|(_, tile)| {
            tile.image_visible & amp;
            &amp;
            !tile.solved
        });

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
                let main_window = main_window_weak.unwrap();
                main_window.set_disable_tiles(true);
                let tiles_model = tiles_model.clone();
                sixtyfps::Timer::single_shot(std::time::Duration::from_secs(1), move || {
                    main_window.set_disable_tiles(false);
                    t1.image_visible = false;
                    tiles_model.set_row_data(t1_idx, t1);
                    t2.image_visible = false;
                    tiles_model.set_row_data(t2_idx, t2);
                });
            }
        }
    });

    main_window.run();
    // ANCHOR_END: game_logic
}
