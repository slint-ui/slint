#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("slide_puzzle.slint", import.meta.url));
const appWindow = new ui.MainWindow();

// --- Game state ---

const pieceCount = 15;
let positions = []; // 16 entries: piece index at each grid cell, -1 = empty
let finished = false;
let autoPlayTimer = null;
let kickAnimationTimer = null;
const kickSpeed = new Array(pieceCount).fill(null).map(() => ({ x: 0, y: 0 }));

const tiles = [];
for (let i = 0; i < pieceCount; i++) {
    tiles.push({ pos_x: 0, pos_y: 0, offset_x: 0, offset_y: 0 });
}
const model = new slint.ArrayModel(tiles);
appWindow.pieces = model;

// --- Solvability check ---
// https://horatiuvlad.com/unitbv/inteligenta_artificiala/2015/TilesSolvability.html

function isSolvable(pos) {
    let inversions = 0;
    for (let x = 0; x < pos.length - 1; x++) {
        const v = pos[x];
        for (let y = x + 1; y < pos.length; y++) {
            if (pos[y] >= 0 && pos[y] < v) inversions++;
        }
    }
    const blankRow = Math.floor(pos.indexOf(-1) / 4);
    return inversions % 2 !== blankRow % 2;
}

function shuffle() {
    const vec = [-1, ...Array.from({ length: 15 }, (_, i) => i)];
    do {
        for (let i = vec.length - 1; i > 0; i--) {
            const j = Math.floor(Math.random() * (i + 1));
            [vec[i], vec[j]] = [vec[j], vec[i]];
        }
    } while (!isSolvable(vec));
    return vec;
}

// --- Piece placement ---

function setPiecePos(p, pos) {
    if (p >= 0) {
        model.setRowData(p, {
            pos_x: Math.floor(pos / 4),
            pos_y: pos % 4,
            offset_x: 0,
            offset_y: 0,
        });
    }
}

function applyTilesLeft() {
    const left = pieceCount - positions.filter((p, i) => p === i).length;
    appWindow.tiles_left = left;
    finished = left === 0;
}

function randomize() {
    positions = shuffle();
    positions.forEach((p, i) => setPiecePos(p, i));
    appWindow.moves = 0;
    applyTilesLeft();
}

// --- Slide logic ---

function slide(pos, offset) {
    let swap = pos;
    while (positions[pos] !== -1) {
        swap += offset;
        [positions[pos], positions[swap]] = [positions[swap], positions[pos]];
        setPiecePos(positions[swap], swap);
    }
}

function pieceClicked(p) {
    const piece = model.rowData(p);
    const pos = piece.pos_x * 4 + piece.pos_y;
    const hole = positions.indexOf(-1);
    const sign = pos > hole ? -1 : 1;

    if (hole % 4 === piece.pos_y) {
        slide(pos, sign * 4);
    } else if (Math.floor(hole / 4) === piece.pos_x) {
        slide(pos, sign);
    } else {
        // Can't move — kick animation
        kickSpeed[p].x = hole % 4 > piece.pos_y ? 10 : -10;
        kickSpeed[p].y = Math.floor(hole / 4) > piece.pos_x ? 10 : -10;
        return false;
    }

    applyTilesLeft();
    appWindow.moves = appWindow.moves + 1;
    return true;
}

// --- Spring animation for invalid moves ---

function springAnimation(piece, axis, speed) {
    const C = 0.3;
    const DAMP = 0.7;
    const EPS = 0.3;

    let offset = axis === "x" ? piece.offset_x : piece.offset_y;
    const acceleration = -offset * C;
    speed += acceleration;
    speed *= DAMP;

    if (speed !== 0 || offset !== 0) {
        offset += speed;
        if (Math.abs(speed) < EPS && Math.abs(offset) < EPS) {
            speed = 0;
            offset = 0;
        }
    }

    if (axis === "x") piece.offset_x = offset;
    else piece.offset_y = offset;

    return speed;
}

function kickAnimation() {
    let hasAnimation = false;
    for (let i = 0; i < pieceCount; i++) {
        const piece = model.rowData(i);
        const prevSpeedX = kickSpeed[i].x;
        const prevSpeedY = kickSpeed[i].y;
        kickSpeed[i].x = springAnimation(piece, "x", kickSpeed[i].x);
        kickSpeed[i].y = springAnimation(piece, "y", kickSpeed[i].y);

        if (
            kickSpeed[i].x !== 0 ||
            kickSpeed[i].y !== 0 ||
            prevSpeedX !== 0 ||
            prevSpeedY !== 0
        ) {
            model.setRowData(i, piece);
            hasAnimation = true;
        }
    }
    if (!hasAnimation && kickAnimationTimer !== null) {
        clearInterval(kickAnimationTimer);
        kickAnimationTimer = null;
    }
}

// --- Auto-play: pick a random valid move ---

function randomMove() {
    const hole = positions.indexOf(-1);
    let cell;
    do {
        cell = Math.floor(Math.random() * 16);
    } while (
        cell === hole ||
        (hole % 4 !== cell % 4 &&
            Math.floor(hole / 4) !== Math.floor(cell / 4))
    );
    pieceClicked(positions[cell]);
}

// --- Callbacks ---

appWindow.piece_clicked = function (p) {
    if (autoPlayTimer !== null) {
        clearInterval(autoPlayTimer);
        autoPlayTimer = null;
        appWindow.auto_play = false;
    }
    if (finished) return;

    if (!pieceClicked(p)) {
        if (kickAnimationTimer !== null) clearInterval(kickAnimationTimer);
        kickAnimationTimer = setInterval(kickAnimation, 16);
    }
};

appWindow.reset = function () {
    if (autoPlayTimer !== null) {
        clearInterval(autoPlayTimer);
        autoPlayTimer = null;
        appWindow.auto_play = false;
    }
    randomize();
};

appWindow.enable_auto_mode = function (enabled) {
    if (enabled) {
        autoPlayTimer = setInterval(randomMove, 200);
    } else if (autoPlayTimer !== null) {
        clearInterval(autoPlayTimer);
        autoPlayTimer = null;
    }
};

// --- Start ---

randomize();
await appWindow.run();
process.exit(0);
