#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

let slint = require("slint-ui");
let ui = require("./memory.slint");
let window = new ui.MainWindow();
// window.image = new slint.Image("icons/bus.png");

const arr = new Uint8ClampedArray(40_000);

// Fill the array with the same RGBA values
for (let i = 0; i < arr.length; i += 4) {
  arr[i + 0] = 0; // R value
  arr[i + 1] = 190; // G value
  arr[i + 2] = 0; // B value
  arr[i + 3] = 255; // A value
}

// Initialize a new ImageData object
let imageData = new slint.ImageData(arr, 200);


window.image = new slint.Image(imageData);

window.run();
