# Game Logic In Rust

We'll implement the rules of the game in Rust as well. The general philosophy of SixtyFPS is that merely the user
interface is implemented in the `.60` language and the business logic in your favorite programming
language. The game rules shall enforce that at most two tiles have their curtain open. If the tiles match, then we
consider them solved and they remain open. Otherwise we wait for a little while, so the player can memorize
the location of the icons, and then close them again.

We'll modify the `.60` markup inside the `sixtyfps!` macro to signal to the Rust code when the user clicks on a tile.
Two changes to `MainWindow` are needed: We need to add a way for the MainWindow to call to the Rust code that it should
check if a pair of tiles has been solved. And we need to add a property that Rust code can toggle to disable further
tile interaction, to prevent the player from opening more tiles than allowed. No cheating allowed! First, we paste
the callback and property declarations into `MainWindow`:


```60
...
MainWindow := Window {
    callback check_if_pair_solved(); // Added
    property <bool> disable_tiles; // Added

    width: 326px;
    height: 326px;

    property <[TileData]> memory_tiles: [
       { image: img!"icons/at.png" },
...
```

The last change to the `.60` markup is to act when the `MemoryTile` signals that it was clicked on. We add the following handler:

```60
...
MainWindow := Window {
    ...
    for tile[i] in memory_tiles : MemoryTile {
        x: mod(i, 4) * 74px;
        y: floor(i / 4) * 74px;
        width: 64px;
        height: 64px;
        icon: tile.image;
        open_curtain: tile.image_visible || tile.solved;
        // propagate the solved status from the model to the tile
        solved: tile.solved;

        clicked => {
            // old: tile.image_visible = !tile.image_visible;
            // new:
            if (!root.disable_tiles) {
                tile.image_visible = !tile.image_visible;
                root.check_if_pair_solved();
            }
        }
    }
}
```

On the Rust side, we can now add an handler to the `check_if_pair_solved` callback, that will check if
two tiles are opened. If they match, the `solved` property is set to true in the model. If they don't
match, start a timer that will close them after one second. While the timer is running, we disable every tile so
one cannot click anything during this time.

Insert this code before the `main_window.run()` call:

```rust
// ...
    let main_window_weak = main_window.as_weak();
    main_window.on_check_if_pair_solved(move || {
        let mut flipped_tiles =
            tiles_model.iter().enumerate().filter(|(_, tile)| {
                tile.image_visible &amp;&amp; !tile.solved
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
                sixtyfps::Timer::single_shot(
                    std::time::Duration::from_secs(1),
                    move || {
                        main_window
                            .set_disable_tiles(false);
                        t1.image_visible = false;
                        tiles_model.set_row_data(t1_idx, t1);
                        t2.image_visible = false;
                        tiles_model.set_row_data(t2_idx, t2);
                    }
                );
            }
        }
    });

    main_window.run();
```

Notice that we take a [Weak](https://sixtyfps.io/docs/rust/sixtyfps/struct.weak) pointer of our `main_window`. This is very
important because capturing a copy of the `main_window` itself within the callback handler would result in a circular ownership.
The `MainWindow` owns the callback handler, which itself owns a reference to the `MainWindow`, which must be weak
instead of strong to avoid a memory leak.

And that's it, now we can run the game!