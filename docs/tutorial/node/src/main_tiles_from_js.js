// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// MIT

// ANCHOR: main
// main.js
let slint = require("slint-ui");
let ui = require("./memory.slint");
let mainWindow = new ui.MainWindow();

let initial_tiles = mainWindow.memory_tiles;
let tiles = initial_tiles.concat(initial_tiles.map((tile) => Object.assign({}, tile)));

for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

let model = new slint.ArrayModel(tiles);
mainWindow.memory_tiles = model;

mainWindow.run();

// ANCHOR_END: main
