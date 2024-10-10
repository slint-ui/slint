#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("memory.slint", import.meta.url));
const window = new ui.MainWindow();

const initial_tiles = [...window.memory_tiles];
const tiles = initial_tiles.concat(
    initial_tiles.map((tile) => Object.assign({}, tile)),
);

for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

const model = new slint.ArrayModel(tiles);
window.memory_tiles = model;

window.check_if_pair_solved = function () {
    const flipped_tiles = [];
    tiles.forEach((tile, index) => {
        if (tile.image_visible && !tile.solved) {
            flipped_tiles.push({
                index,
                tile,
            });
        }
    });

    if (flipped_tiles.length === 2) {
        const { tile: tile1, index: tile1_index } = flipped_tiles[0];

        const { tile: tile2, index: tile2_index } = flipped_tiles[1];

        const is_pair_solved = tile1.image.path === tile2.image.path;
        if (is_pair_solved) {
            tile1.solved = true;
            model.setRowData(tile1_index, tile1);
            tile2.solved = true;
            model.setRowData(tile2_index, tile2);
        } else {
            window.disable_tiles = true;
            setTimeout(() => {
                window.disable_tiles = false;
                tile1.image_visible = false;
                model.setRowData(tile1_index, tile1);
                tile2.image_visible = false;
                model.setRowData(tile2_index, tile2);
            }, 1000);
        }
    }
};

window.run();
