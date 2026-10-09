// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const source = await readFile(new URL("../preview/zoom.mjs", import.meta.url), "utf8");

function preview(initialSize = { width: 800, height: 500 }) {
  const listeners = new Map();
  const body = { dataset: {} };
  const viewport = {
    style: {}, clientWidth: 720, availableHeight: 1024, scrollLeft: 0, scrollTop: 0,
    get clientHeight() { return this.style.height ? Number.parseFloat(this.style.height) : this.availableHeight; },
    getBoundingClientRect() { return { left: 0, top: 0 }; },
    addEventListener(name, callback) { listeners.set(name, callback); },
  };
  const content = { style: {} };
  const stage = { style: {} };
  const canvas = { getBoundingClientRect() { return { left: 12, top: 12 }; } };
  const elements = { "preview-scroll": viewport, "preview-content": content, "preview-stage": stage };
  let resize;
  const context = {
    document: { body, getElementById: name => elements[name] },
    ResizeObserver: class {
      constructor(callback) { resize = callback; }
      observe(element) { assert.equal(element, viewport); }
    },
  };
  const install = runInNewContext(source.replace("export function", "function") + "\ninstallPreviewZoom;", context);
  const zoom = install({ canvas, initialSize });
  return {
    zoom, viewport, body,
    dimensions() { return [Number.parseFloat(content.style.width), Number.parseFloat(content.style.height)]; },
    resize(width, height) {
      viewport.clientWidth = width;
      viewport.availableHeight = height;
      resize();
    },
    resetZoom() {
      listeners.get("keydown")({ key: "0", ctrlKey: true, preventDefault() {}, stopPropagation() {} });
    },
  };
}

test("a separate tab uses its full width and height and returns to compact inline sizing", () => {
  const view = preview();
  const inlineDimensions = view.dimensions();
  assert.equal(inlineDimensions[1], 396);
  assert.equal(view.viewport.style.height, "420px");
  view.resize(1624, 1024);
  view.zoom.setDisplayMode("fullscreen");
  assert.equal(view.body.dataset.displayMode, "fullscreen");
  assert.equal(view.viewport.style.height, "");
  assert.deepEqual(view.dimensions(), [1600, 1000]);
  for (const [width, height] of [[1624, 524], [824, 1024]]) {
    view.resize(width, height);
    assert.deepEqual(view.dimensions(), [800, 500]);
  }
  view.resize(720, 1024);
  view.zoom.setDisplayMode("inline");
  assert.equal(view.body.dataset.displayMode, "inline");
  assert.deepEqual(view.dimensions(), inlineDimensions);
  assert.equal(view.viewport.style.height, "420px");
});

test("small previews grow in a tab while explicit keyboard zoom survives host updates and resizing", () => {
  const view = preview({ width: 200, height: 100 });
  assert.deepEqual(view.dimensions(), [200, 100]);
  view.resize(824, 1024);
  view.zoom.setDisplayMode("fullscreen");
  assert.deepEqual(view.dimensions(), [800, 400]);
  view.resetZoom();
  view.zoom.setDisplayMode("fullscreen");
  view.resize(1624, 1024);
  assert.deepEqual(view.dimensions(), [200, 100]);
  assert.equal(view.viewport.style.height, "");
  view.zoom.setDisplayMode("inline");
  assert.deepEqual(view.dimensions(), [200, 100]);
});

test("a hidden fullscreen preview fits when it becomes visible or its source size changes", () => {
  const view = preview();
  view.resize(0, 0);
  view.zoom.setDisplayMode("fullscreen");
  view.resize(1624, 1024);
  assert.deepEqual(view.dimensions(), [1600, 1000]);
  view.zoom.setSize(400, 250);
  assert.deepEqual(view.dimensions(), [1600, 1000]);
});
