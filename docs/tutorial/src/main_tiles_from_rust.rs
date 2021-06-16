/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
// ANCHOR: tiles
fn main() {
    use sixtyfps::Model;

    let main_window = MainWindow::new();

    // Fetch the tiles from the model
    let mut tiles: Vec<TileData> =
        main_window.get_memory_tiles().iter().collect();
    // Duplicate them to ensure that we have pairs
    tiles.extend(tiles.clone());

    // Randomly mix the tiles
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    tiles.shuffle(&amp;mut rng);

    // Assign the shuffled Vec to the model property
    let tiles_model =
        std::rc::Rc::new(sixtyfps::VecModel::from(tiles));
    main_window.set_memory_tiles(
        sixtyfps::ModelHandle::new(tiles_model.clone()));

    main_window.run();
}
// ANCHOR_END: tiles