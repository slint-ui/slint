// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

// Exercise the actual bundled sandbox's event wiring. Detailed node capture,
// normalization and conversion belong to their unit tests and the real UI worker suite.
const code = await readFile(
    new URL("../dist/code.js", import.meta.url),
    "utf8",
);
const fixture = JSON.parse(
    await readFile(
        new URL("../fixtures/source/basic.json", import.meta.url),
        "utf8",
    ),
);
const squarePng = new Uint8Array(
    await readFile(new URL("../fixtures/authored/square.png", import.meta.url)),
);
function events() {
    const listeners = new Map();
    return {
        on: (name, fn) => listeners.set(name, fn),
        off: (name) => listeners.delete(name),
        emit: (name, event) => listeners.get(name)?.(event),
        has: (name) => listeners.has(name),
    };
}
function boot({ forceFallback = false, forceRasterFallback = false } = {}) {
    const messages = [],
        global = events(),
        first = events(),
        second = events();
    const node = {
        ...fixture.root.properties,
        id: "authored:root",
        name: "Root",
        type: "RECTANGLE",
        removed: false,
    };
    if (forceFallback)
        Object.assign(node, {
            type: "TEXT",
            characters: "cloud_upload",
            fontName: {
                family: "Material Symbols Outlined",
                style: "Regular",
            },
            fontSize: 24,
            fontWeight: 400,
            textAlignHorizontal: "CENTER",
            textAlignVertical: "CENTER",
            textAutoResize: "NONE",
            hasMissingFont: false,
            getStyledTextSegments: () => [],
            exportAsync: () => new Promise(() => {}),
        });
    if (forceRasterFallback)
        Object.assign(node, {
            fills: [
                {
                    type: "IMAGE",
                    visible: true,
                    opacity: 1,
                    blendMode: "NORMAL",
                    imageHash: "slow-image",
                    scaleMode: "FILL",
                },
            ],
            exportAsync: async () => squarePng,
        });
    const other = { ...node, id: "authored:other", name: "Other" };
    Object.assign(first, { id: "page:1", type: "PAGE", selection: [node] });
    Object.assign(second, { id: "page:2", type: "PAGE", selection: [other] });
    node.parent = first;
    other.parent = second;
    const figma = {
        mixed: Symbol("mixed"),
        currentPage: first,
        on: global.on,
        skipInvisibleInstanceChildren: false,
        clientStorage: {
            getAsync: async () => undefined,
            setAsync: async () => {},
        },
        showUI() {},
        ui: {
            onmessage: undefined,
            resize() {},
            postMessage: (message) => messages.push(message),
        },
        getNodeByIdAsync: async (id) => (id === node.id ? node : null),
        getImageByHash: () => ({
            getBytesAsync: () => new Promise(() => {}),
            getSizeAsync: async () => ({ width: 32, height: 32 }),
        }),
    };
    // Conversion is bundled for native codegen but must never run in preview mode.
    const previewCode = code.replace(
        /function (?:createSourceNormalizer|convertSnapshot)\([^)]*\) \{/g,
        '$& throw Error("Preview attempted sandbox conversion");',
    );
    assert.notEqual(previewCode, code);
    const sandboxSetTimeout =
        forceFallback || forceRasterFallback
            ? (callback, delay, ...args) =>
                  setTimeout(callback, delay === 20_000 ? 0 : delay, ...args)
            : setTimeout;
    vm.runInNewContext(previewCode, {
        figma,
        __html__: "",
        console,
        setTimeout: sandboxSetTimeout,
        clearTimeout,
        Uint8Array,
        TextEncoder,
        TextDecoder,
    });
    return {
        figma,
        messages,
        global,
        first,
        second,
        node,
        other,
    };
}
async function until(predicate) {
    const deadline = Date.now() + 3000;
    while (!predicate()) {
        if (Date.now() > deadline)
            throw Error("Sandbox did not emit expected message");
        await new Promise((resolve) => setTimeout(resolve, 5));
    }
}
const state = boot();
const { figma, messages, global, first, node, other } = state;
const send = (message) => figma.ui.onmessage(message);
const count = (type) => messages.filter((m) => m.type === type).length;
send({ type: "ui-ready", devicePixelRatio: 1 });
await until(() => count("preview-capture") === 1);
const initial = messages.find((m) => m.type === "preview-capture");
assert.equal(JSON.parse(initial.captureJson).root.id, node.id);
assert.equal(initial.trace.revision, initial.revision);
assert.equal(
    messages.find((m) => m.type === "preview-busy").revision,
    initial.revision,
);
send({ type: "ui-ready", devicePixelRatio: 1 });
assert.equal(count("preview-capture"), 1);
send({ type: "pin-selection" });
await until(() => count("preview-capture") === 2);
assert.equal(
    messages.filter((m) => m.type === "pin-state").at(-1).pinnedRoot.nodeId,
    node.id,
);
first.selection = [other];
global.emit("selectionchange");
assert.equal(count("preview-capture"), 2);
send({ type: "unpin" });
await until(() => count("preview-capture") === 3);
assert.equal(
    JSON.parse(
        messages.filter((m) => m.type === "preview-capture").at(-1).captureJson,
    ).root.id,
    other.id,
);
first.selection = [];
global.emit("selectionchange");
await until(() => count("preview-clear") === 1);
assert.equal(messages.at(-1).type, "preview-clear");
first.selection = [node];
global.emit("selectionchange");
await until(() => count("preview-capture") === 4);
send({ type: "pin-selection" });
await until(() => count("preview-capture") === 5);
node.removed = true;
first.emit("nodechange", { nodeChanges: [{ type: "DELETE", id: node.id }] });
await until(
    () =>
        messages.filter((m) => m.type === "pin-state").at(-1).pinned === false,
);
node.removed = false;
first.selection = [node];
send({ type: "pin-selection" });
await until(() => messages.filter((m) => m.type === "pin-state").at(-1).pinned);
figma.currentPage = state.second;
global.emit("currentpagechange");
await until(
    () =>
        messages.filter((m) => m.type === "pin-state").at(-1).pinned === false,
);
assert.equal(first.has("nodechange"), false);
assert.equal(state.second.has("nodechange"), true);
const restarted = boot();
restarted.figma.ui.onmessage({ type: "ui-ready", devicePixelRatio: 1 });
await until(() => restarted.messages.some((m) => m.type === "pin-state"));
assert.equal(
    restarted.messages.find((m) => m.type === "pin-state").pinned,
    false,
);
const slow = boot({ forceFallback: true });
slow.figma.ui.onmessage({ type: "ui-ready", devicePixelRatio: 1 });
await until(() =>
    slow.messages.some((message) => message.type === "preview-capture"),
);
const slowCapture = slow.messages.find(
    (message) => message.type === "preview-capture",
);
const slowBusy = slow.messages.filter(
    (message) => message.type === "preview-busy",
);
assert.equal(slowBusy.length, 2);
assert.equal(
    slowBusy[1].message,
    "This selection is large. Trying a simpler export.",
);
assert.equal(JSON.parse(slowCapture.captureJson).flattened, true);
assert.equal(JSON.parse(slowCapture.captureJson).components, undefined);
assert.equal(JSON.parse(slowCapture.captureJson).root.exports, undefined);
assert.equal(slowCapture.warnings[0].code, "SIMPLIFIED_CAPTURE");
const raster = boot({ forceRasterFallback: true });
raster.figma.ui.onmessage({ type: "ui-ready", devicePixelRatio: 1 });
await until(() =>
    raster.messages.some((message) => message.type === "preview-capture"),
);
const rasterCapture = raster.messages.find(
    (message) => message.type === "preview-capture",
);
const rasterSource = JSON.parse(rasterCapture.captureJson);
assert.equal(rasterSource.flattened, undefined);
assert.equal(rasterSource.root.type, "VECTOR");
assert.equal(rasterSource.root.children, undefined);
assert.notEqual(rasterSource.root.exports.png.value, undefined);
assert.equal(rasterCapture.warnings[0].code, "SIMPLIFIED_CAPTURE");
// Pinning retains a selection independently of how the converter renders it.
for (const type of [
    "TABLE",
    "CONNECTOR",
    "SHAPE_WITH_TEXT",
    "SLICE",
    "INSTANCE",
]) {
    const pinned = boot();
    pinned.node.type = type;
    pinned.node.scaleFactor = 2;
    pinned.global.emit("selectionchange");
    assert.equal(pinned.messages.at(-1).canPin, true, type);
    pinned.figma.ui.onmessage({ type: "pin-selection" });
    assert.equal(
        pinned.messages.at(-1).pinnedRoot.nodeId,
        pinned.node.id,
        type,
    );
}
for (const kind of ["empty", "multiple", "removed"]) {
    const invalid = boot();
    if (kind === "empty") invalid.first.selection = [];
    else if (kind === "multiple")
        invalid.first.selection = [invalid.node, invalid.other];
    else invalid.node.removed = true;
    invalid.global.emit("selectionchange");
    assert.equal(invalid.messages.at(-1).canPin, false, kind);
    invalid.figma.ui.onmessage({ type: "pin-selection" });
    assert.equal(invalid.messages.at(-1).pinned, false, kind);
}
console.log(
    "Validated built sandbox handshake, capture revisions, selection clearing and pin lifecycle",
);

// Dev Mode must start without touching preview-only host APIs.
for (const directory of ["dist", "dist-dev"]) {
    const manifest = JSON.parse(
        await readFile(
            new URL(`../${directory}/manifest.json`, import.meta.url),
            "utf8",
        ),
    );
    assert.equal(manifest.id, "1474418299182276871");
    assert.equal(manifest.name, "Figma to Slint");
    assert.equal(manifest.codegenPreferences[0].propertyName, "useVariables");
    assert.deepEqual(manifest.codegenPreferences[0].includedLanguages, [
        "slint",
    ]);
    assert.equal(
        manifest.codegenPreferences[0].options.find(
            (option) => option.isDefault,
        ).value,
        "false",
    );
    assert.deepEqual(manifest.editorType, ["figma", "dev"]);
    assert.deepEqual(manifest.capabilities, ["codegen", "vscode"]);
    assert.deepEqual(manifest.codegenLanguages, [
        { label: "Slint", value: "slint" },
    ]);
    let generate;
    const forbidden = () => {
        throw Error("Preview initialized during codegen");
    };
    const host = {
        editorType: "dev",
        mode: "codegen",
        mixed: Symbol("mixed"),
        codegen: {
            on(event, callback) {
                assert.equal(event, "generate");
                generate = callback;
            },
        },
        get currentPage() {
            return forbidden();
        },
        get clientStorage() {
            return forbidden();
        },
        get ui() {
            return forbidden();
        },
        showUI: forbidden,
        on: forbidden,
    };
    vm.runInNewContext(
        await readFile(
            new URL(`../${directory}/code.js`, import.meta.url),
            "utf8",
        ),
        { figma: host, console, setTimeout, clearTimeout, Uint8Array },
    );
    assert.equal(typeof generate, "function");
    const root = {
        ...fixture.root.properties,
        id: "codegen:root",
        name: "Codegen root",
        type: "RECTANGLE",
    };
    const failure = await generate({
        node: { ...root, width: -1 },
        language: "slint",
    });
    assert.equal(failure[0].language, "PLAINTEXT");
    const [a, b] = await Promise.all([
        generate({ node: root, language: "slint" }),
        generate({
            node: { ...root, name: "Other", width: 211 },
            language: "slint",
        }),
    ]);
    assert.equal(a[0].title, "Slint Code: Codegen root");
    assert.equal(a[0].language, "CSS");
    assert.match(a[0].code, /width: 100px;/);
    assert.match(b[0].code, /width: 211px;/);
    assert.equal(b[0].title, "Slint Code: Other");
    assert.doesNotMatch(a[0].code, /export component/);
}
console.log(
    "Validated production and development native root-only codegen startup and request isolation",
);
