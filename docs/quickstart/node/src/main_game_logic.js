// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT


// main.js
import * as slint from "slint-ui";

let ui = slint.loadFile("./memory.slint");
let mainWindow = new ui.MainWindow();

let initial_tiles = [...mainWindow.memory_tiles];
let tiles = initial_tiles.concat(initial_tiles.map((tile) => Object.assign({}, tile)));

for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

let model = new slint.ArrayModel(tiles);
mainWindow.memory_tiles = model;

// ANCHOR: game_logic
mainWindow.check_if_pair_solved = function () {
    let flipped_tiles = [];
    tiles.forEach((tile, index) => {
        if (tile.image_visible && !tile.solved) {
            flipped_tiles.push({
                index,
                tile
            });
        }
    });

    if (flipped_tiles.length == 2) {
        let {
            tile: tile1,
            index: tile1_index
        } = flipped_tiles[0];

        let {
            tile: tile2,
            index: tile2_index
        } = flipped_tiles[1];

        let is_pair_solved = tile1.image.path === tile2.image.path;
        if (is_pair_solved) {
            tile1.solved = true;
            model.setRowData(tile1_index, tile1);
            tile2.solved = true;
            model.setRowData(tile2_index, tile2);
        } else {
            mainWindow.disable_tiles = true;
            setTimeout(() => {
                mainWindow.disable_tiles = false;
                tile1.image_visible = false;
                model.setRowData(tile1_index, tile1);
                tile2.image_visible = false;
                model.setRowData(tile2_index, tile2);
            }, 1000)

        }
    }
};
// ANCHOR_END: game_logic
await mainWindow.run();