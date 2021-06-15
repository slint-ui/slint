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