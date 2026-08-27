#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
import sharp from "sharp";

const ui = slint.loadFile(new URL("ui/main.slint", import.meta.url));
const appWindow = new ui.MainUI();

const TILE_SIZE = 256;
const OSM_URL =
    process.env.OSM_TILES_URL || "https://tile.openstreetmap.org";

// --- Tile cache and loading state ---

// Key: "z/x/y"
const loadedTiles = new Map();
const loadingTiles = new Map();

function tileKey(z, x, y) {
    return `${z}/${x}/${y}`;
}

async function fetchTile(z, x, y) {
    const url = `${OSM_URL}/${z}/${x}/${y}.png`;
    try {
        const resp = await fetch(url, {
            headers: { "User-Agent": "Slint Maps example" },
        });
        if (!resp.ok) {
            console.error(`Error loading ${url}: ${resp.status}`);
            return null;
        }
        const bytes = await resp.arrayBuffer();
        const { data, info } = await sharp(Buffer.from(bytes))
            .resize(TILE_SIZE, TILE_SIZE, { fit: "fill" })
            .ensureAlpha()
            .raw()
            .toBuffer({ resolveWithObject: true });
        return { width: info.width, height: info.height, data };
    } catch (err) {
        console.error(`Error loading ${url}: ${err.message}`);
        return null;
    }
}

// --- World state ---

let zoomLevel = 1;
let offsetX = 0;
let offsetY = 0;
let visibleWidth = 0;
let visibleHeight = 0;

const tileModel = new slint.ArrayModel([]);
appWindow.tiles = tileModel;

function setZoomLevel(newZoom, ox, oy) {
    newZoom = Math.max(1, Math.min(19, Math.round(newZoom)));
    if (newZoom === zoomLevel) return;

    loadedTiles.clear();
    loadingTiles.clear();

    const scale = 2 ** (newZoom - zoomLevel);
    offsetX = (offsetX + ox) * scale - ox;
    offsetY = (offsetY + oy) * scale - oy;
    zoomLevel = newZoom;
    resetView();
}

function resetView() {
    const m = 1 << zoomLevel;
    const minX = Math.floor(offsetX / TILE_SIZE);
    const minY = Math.floor(offsetY / TILE_SIZE);
    const maxX = Math.min(
        Math.ceil((offsetX + visibleWidth) / TILE_SIZE) + 1,
        m,
    );
    const maxY = Math.min(
        Math.ceil((offsetY + visibleHeight) / TILE_SIZE) + 1,
        m,
    );

    const KEEP = 10;
    for (const [key, _] of loadedTiles) {
        const [z, x, y] = key.split("/").map(Number);
        if (
            z !== zoomLevel ||
            x <= minX - KEEP ||
            x >= maxX + KEEP ||
            y <= minY - KEEP ||
            y >= maxY + KEEP
        ) {
            loadedTiles.delete(key);
        }
    }
    for (const key of loadingTiles.keys()) {
        const [z, x, y] = key.split("/").map(Number);
        if (
            z !== zoomLevel ||
            x <= minX - KEEP ||
            x >= maxX + KEEP ||
            y <= minY - KEEP ||
            y >= maxY + KEEP
        ) {
            loadingTiles.delete(key);
        }
    }

    for (let x = minX; x < maxX; x++) {
        for (let y = minY; y < maxY; y++) {
            const key = tileKey(zoomLevel, x, y);
            if (loadedTiles.has(key) || loadingTiles.has(key)) continue;

            const promise = fetchTile(zoomLevel, x, y).then((img) => {
                loadingTiles.delete(key);
                if (img) {
                    loadedTiles.set(key, { x, y, img });
                    refreshModel();
                }
            });
            loadingTiles.set(key, promise);
        }
    }
}

function refreshModel() {
    const items = [];
    for (const [_, entry] of loadedTiles) {
        items.push({
            x: entry.x * TILE_SIZE,
            y: entry.y * TILE_SIZE,
            tile: entry.img,
        });
    }

    const oldCount = tileModel.rowCount();
    const shared = Math.min(oldCount, items.length);
    for (let i = 0; i < shared; i++) tileModel.setRowData(i, items[i]);
    for (let i = oldCount - 1; i >= shared; i--) tileModel.remove(i, 1);
    for (let i = shared; i < items.length; i++) tileModel.push(items[i]);
}

function setViewportSize() {
    appWindow.zoom = zoomLevel;
    const worldSize = TILE_SIZE * (1 << zoomLevel);
    appWindow.set_viewport(-offsetX, -offsetY, worldSize, worldSize);
}

// --- Wire up callbacks ---

appWindow.flicked = function (ox, oy) {
    offsetX = -ox;
    offsetY = -oy;
    visibleWidth = appWindow.visible_width;
    visibleHeight = appWindow.visible_height;
    resetView();
    refreshModel();
};

appWindow.zoom_changed = function (zoom) {
    visibleWidth = appWindow.visible_width;
    visibleHeight = appWindow.visible_height;
    setZoomLevel(zoom, visibleWidth / 2, visibleHeight / 2);
    setViewportSize();
    refreshModel();
};

appWindow.zoom_in = function (ox, oy) {
    visibleWidth = appWindow.visible_width;
    visibleHeight = appWindow.visible_height;
    setZoomLevel(zoomLevel + 1, ox, oy);
    setViewportSize();
    refreshModel();
};

appWindow.zoom_out = function (ox, oy) {
    visibleWidth = appWindow.visible_width;
    visibleHeight = appWindow.visible_height;
    setZoomLevel(zoomLevel - 1, ox, oy);
    setViewportSize();
    refreshModel();
};

appWindow.link_clicked = function () {
    import("node:child_process").then(({ exec }) => {
        exec("xdg-open https://www.openstreetmap.org/copyright");
    });
};

// --- Initial load (deferred until the event loop provides layout dimensions) ---

let initialized = false;
const initTimer = setInterval(() => {
    const vw = appWindow.visible_width;
    const vh = appWindow.visible_height;
    if (vw > 0 && vh > 0 && !initialized) {
        initialized = true;
        clearInterval(initTimer);
        visibleWidth = vw;
        visibleHeight = vh;
        resetView();
        setViewportSize();
    }
}, 16);

await appWindow.run();
process.exit(0);
