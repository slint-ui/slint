// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// ANCHOR: main
// main.js
import * as slint from "slint-ui";
const ui = slint.loadFile(new URL("./ui/app-window.slint", import.meta.url));
const mainWindow = new ui.MainWindow();

const initial_tiles = mainWindow.memory_tiles;
const tiles = initial_tiles.concat(
    initial_tiles.map((tile) => Object.assign({}, tile)),
);

for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

const model = new slint.ArrayModel(tiles);
mainWindow.memory_tiles = model;

await mainWindow.run();

// ANCHOR_END: main
