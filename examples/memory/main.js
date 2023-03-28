#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

let slint = require("slint-ui");
let ui = require("./memory.slint");
let window = new ui.MainWindow();

let initial_tiles = window.memory_tiles;
let tiles = initial_tiles.concat(initial_tiles.map((tile) => Object.assign({}, tile)));

for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

let model = new slint.ArrayModel(tiles);
window.memory_tiles = model;

window.check_if_pair_solved.setHandler(function () {
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

        let is_pair_solved = tile1.image === tile2.image;
        if (is_pair_solved) {
            tile1.solved = true;
            model.setRowData(tile1_index, tile1);
            tile2.solved = true;
            model.setRowData(tile2_index, tile2);
        } else {
            window.disable_tiles = true;
            slint.Timer.singleShot(1000, () => {
                window.disable_tiles = false;
                tile1.image_visible = false;
                model.setRowData(tile1_index, tile1);
                tile2.image_visible = false;
                model.setRowData(tile2_index, tile2);
            });

        }
    }
})

window.run();
