// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * Registry of demos the playground can load. Each demo points at an existing
 * example directory in the workspace (so .slint sources and binary assets
 * stay in sync with what we ship for Node/Rust) and ships its own
 * playground-flavoured `main.js`.
 *
 * The base directories are absolute filesystem paths exposed by vite at
 * build time (see ../vite.config.mts) and used to build `/@fs/` URLs.
 */

const env = import.meta.env as Record<string, string>;

export interface Demo {
    id: string;
    label: string;
    /** Absolute filesystem path used to build `/@fs/` URLs. */
    baseDir: string;
    /** `.slint` files (relative to baseDir) opened as tabs. The first entry is the main one. */
    slintFiles: string[];
    /** Playground-compatible main.js seed. */
    mainJs: string;
    /**
     * Canvas drawing-buffer size (in CSS pixels) at first render. CSS in
     * preview.html caps the displayed size to fit the preview pane while
     * preserving this aspect ratio.
     */
    preferredWidth: number;
    preferredHeight: number;
}

const PRINTER_JS = `// 'slint' and 'playground' are globals injected by the playground iframe.
// Edit any file in the tabs; press Ctrl+S (or wait ~600ms after you stop
// typing) to re-run.

const mainSource = await playground.readFile(playground.mainSlint);

const ui = await slint.loadSource(mainSource, playground.mainUrl, {
    fileLoader: playground.fileLoader,
});

slint.setCanvasId("canvas");
const appWindow = new ui.MainWindow();

appWindow.PrinterState.ink_levels = [
    { color: "#00ffff", level: 0.3 },
    { color: "#ff00ff", level: 0.8 },
    { color: "#ffff00", level: 0.6 },
    { color: "#000000", level: 0.9 },
];

const printerQueue = new slint.ArrayModel(
    Array.from(appWindow.PrinterQueue.printer_queue),
);
appWindow.PrinterQueue.printer_queue = printerQueue;

appWindow.PrinterQueue.start_job = (title) => {
    const now = new Date();
    const pad = (n) => String(n).padStart(2, "0");
    const date =
        \`\${pad(now.getHours())}:\${pad(now.getMinutes())}:\${pad(now.getSeconds())} \` +
        \`\${pad(now.getDate())}/\${pad(now.getMonth() + 1)}/\${now.getFullYear()}\`;

    printerQueue.push({
        status: "waiting",
        progress: 0,
        title,
        owner: "user@example.com",
        pages: 1,
        size: "100kB",
        submission_date: date,
    });
};

appWindow.PrinterQueue.cancel_job = (index) => {
    printerQueue.remove(index, 1);
};

const progressTimer = setInterval(() => {
    if (printerQueue.length > 0) {
        const top = printerQueue.rowData(0);
        top.progress += 1;
        if (top.progress > 100) {
            printerQueue.remove(0, 1);
        } else {
            top.status = "printing";
            printerQueue.setRowData(0, top);
        }
    }
}, 1000);

await appWindow.run();
clearInterval(progressTimer);
`;

const SLIDE_PUZZLE_JS = `// Slide puzzle — 'slint' and 'playground' are globals injected by the iframe.

const mainSource = await playground.readFile(playground.mainSlint);
const ui = await slint.loadSource(mainSource, playground.mainUrl, {
    fileLoader: playground.fileLoader,
});

slint.setCanvasId("canvas");
const appWindow = new ui.MainWindow();

const pieceCount = 15;
let positions = [];
let finished = false;
let autoPlayTimer = null;
let kickAnimationTimer = null;
const kickSpeed = Array.from({ length: pieceCount }, () => ({ x: 0, y: 0 }));

const tiles = Array.from({ length: pieceCount }, () => ({
    pos_x: 0, pos_y: 0, offset_x: 0, offset_y: 0,
}));
const model = new slint.ArrayModel(tiles);
appWindow.pieces = model;

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

function setPiecePos(p, pos) {
    if (p >= 0) {
        model.setRowData(p, {
            pos_x: Math.floor(pos / 4),
            pos_y: pos % 4,
            offset_x: 0, offset_y: 0,
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

function slide(pos, offset) {
    while (positions[pos] !== -1) {
        const swap = pos + offset;
        [positions[pos], positions[swap]] = [positions[swap], positions[pos]];
        setPiecePos(positions[swap], swap);
        pos = swap;
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
        kickSpeed[p].x = hole % 4 > piece.pos_y ? 10 : -10;
        kickSpeed[p].y = Math.floor(hole / 4) > piece.pos_x ? 10 : -10;
        return false;
    }
    applyTilesLeft();
    appWindow.moves += 1;
    return true;
}

function springAnimation(piece, axis, speed) {
    const C = 0.3, DAMP = 0.7, EPS = 0.3;
    let offset = axis === "x" ? piece.offset_x : piece.offset_y;
    speed = (speed + (-offset * C)) * DAMP;
    if (speed !== 0 || offset !== 0) {
        offset += speed;
        if (Math.abs(speed) < EPS && Math.abs(offset) < EPS) {
            speed = 0; offset = 0;
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
        const prevSx = kickSpeed[i].x, prevSy = kickSpeed[i].y;
        kickSpeed[i].x = springAnimation(piece, "x", kickSpeed[i].x);
        kickSpeed[i].y = springAnimation(piece, "y", kickSpeed[i].y);
        if (kickSpeed[i].x || kickSpeed[i].y || prevSx || prevSy) {
            model.setRowData(i, piece);
            hasAnimation = true;
        }
    }
    if (!hasAnimation && kickAnimationTimer !== null) {
        clearInterval(kickAnimationTimer);
        kickAnimationTimer = null;
    }
}

function randomMove() {
    const hole = positions.indexOf(-1);
    let cell;
    do {
        cell = Math.floor(Math.random() * 16);
    } while (
        cell === hole ||
        (hole % 4 !== cell % 4 && Math.floor(hole / 4) !== Math.floor(cell / 4))
    );
    pieceClicked(positions[cell]);
}

appWindow.piece_clicked = (p) => {
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

appWindow.reset = () => {
    if (autoPlayTimer !== null) {
        clearInterval(autoPlayTimer);
        autoPlayTimer = null;
        appWindow.auto_play = false;
    }
    randomize();
};

appWindow.enable_auto_mode = (enabled) => {
    if (enabled) autoPlayTimer = setInterval(randomMove, 200);
    else if (autoPlayTimer !== null) {
        clearInterval(autoPlayTimer);
        autoPlayTimer = null;
    }
};

randomize();
await appWindow.run();
if (autoPlayTimer !== null) clearInterval(autoPlayTimer);
if (kickAnimationTimer !== null) clearInterval(kickAnimationTimer);
`;

const MEMORY_JS = `// Memory game — 'slint' and 'playground' are globals injected by the iframe.

const mainSource = await playground.readFile(playground.mainSlint);
const ui = await slint.loadSource(mainSource, playground.mainUrl, {
    fileLoader: playground.fileLoader,
});

slint.setCanvasId("canvas");
const appWindow = new ui.MainWindow();

const initial_tiles = [...appWindow.memory_tiles];
const tiles = initial_tiles.concat(
    initial_tiles.map((t) => Object.assign({}, t)),
);
for (let i = tiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * i);
    [tiles[i], tiles[j]] = [tiles[j], tiles[i]];
}

const model = new slint.ArrayModel(tiles);
appWindow.memory_tiles = model;

appWindow.check_if_pair_solved = () => {
    const flipped = [];
    tiles.forEach((tile, index) => {
        if (tile.image_visible && !tile.solved) flipped.push({ index, tile });
    });
    if (flipped.length !== 2) return;
    const { tile: a, index: ai } = flipped[0];
    const { tile: b, index: bi } = flipped[1];
    if (a.image.path === b.image.path) {
        a.solved = true; model.setRowData(ai, a);
        b.solved = true; model.setRowData(bi, b);
    } else {
        appWindow.disable_tiles = true;
        setTimeout(() => {
            appWindow.disable_tiles = false;
            a.image_visible = false; model.setRowData(ai, a);
            b.image_visible = false; model.setRowData(bi, b);
        }, 1000);
    }
};

await appWindow.run();
`;

const TODO_JS = `// Todo — 'slint' and 'playground' are globals injected by the iframe.

const mainSource = await playground.readFile(playground.mainSlint);
const ui = await slint.loadSource(mainSource, playground.mainUrl, {
    fileLoader: playground.fileLoader,
});

slint.setCanvasId("canvas");
const app = new ui.MainWindow();

const model = new slint.ArrayModel([
    { title: "Implement the .slint file", checked: true },
    { title: "Do the Rust part", checked: false },
    { title: "Make the C++ code", checked: false },
    { title: "Write some JavaScript code", checked: true },
    { title: "Test the application", checked: false },
    { title: "Ship to customer", checked: false },
    { title: "???", checked: false },
    { title: "Profit", checked: false },
]);
app.todo_model = model;

app.todo_added = (text) => {
    model.push({ title: text, checked: false });
};

app.remove_done = () => {
    let offset = 0;
    const length = model.length;
    for (let i = 0; i < length; ++i) {
        if (model.rowData(i - offset).checked) {
            model.remove(i - offset, 1);
            offset++;
        }
    }
};

await app.run();
`;

export const DEMOS: ReadonlyArray<Demo> = [
    {
        id: "printer",
        label: "Printer Demo",
        baseDir: env.PLAYGROUND_PRINTER_DEMO_UI_DIR,
        slintFiles: [
            "printerdemo.slint",
            "common.slint",
            "pages/pages.slint",
            "pages/page.slint",
            "pages/home_page.slint",
            "pages/ink_page.slint",
            "pages/print_page.slint",
            "pages/copy_page.slint",
            "pages/scan_page.slint",
            "pages/usb_page.slint",
            "pages/settings_page.slint",
            "pages/printer_queue.slint",
            "components/sidebar.slint",
            "components/button.slint",
            "components/text_button.slint",
            "components/icon_button.slint",
            "components/icon_text_button.slint",
            "components/spinbox.slint",
            "components/headers.slint",
            "components/drop_down_menu.slint",
            "components/popup_menu.slint",
        ],
        mainJs: PRINTER_JS,
        preferredWidth: 900,
        preferredHeight: 600,
    },
    {
        id: "slide-puzzle",
        label: "Slide Puzzle",
        baseDir: env.PLAYGROUND_SLIDE_PUZZLE_DIR,
        slintFiles: ["slide_puzzle.slint"],
        mainJs: SLIDE_PUZZLE_JS,
        preferredWidth: 480,
        preferredHeight: 560,
    },
    {
        id: "memory",
        label: "Memory Game",
        baseDir: env.PLAYGROUND_MEMORY_DIR,
        slintFiles: ["memory.slint"],
        mainJs: MEMORY_JS,
        preferredWidth: 500,
        preferredHeight: 500,
    },
    {
        id: "todo",
        label: "Todo",
        baseDir: env.PLAYGROUND_TODO_UI_DIR,
        slintFiles: ["todo.slint"],
        mainJs: TODO_JS,
        preferredWidth: 400,
        preferredHeight: 600,
    },
];

export const DEFAULT_DEMO_ID = "printer";
