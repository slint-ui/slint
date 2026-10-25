// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const html = readFileSync(new URL("../view.html", import.meta.url), "utf8");
const script = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1]
    .replace("__INTERPRETER_URL__", "https://example.com/interpreter.js")
    .replace("__CHANNEL__", "snapshot");

function startView({ fetch, captureError } = {}) {
    const published = [];
    let onMessage;
    let resolveCapture;
    const captured = new Promise((resolve) => { resolveCapture = resolve; });
    const element = () => ({
        style: { setProperty() {} }, dataset: {},
        classList: { add() {}, remove() {} },
        textContent: "", className: "", children: [], hidden: false,
        width: 320, height: 160, offsetWidth: 320, offsetHeight: 160,
        clientWidth: 320, scrollWidth: 320,
        cloneNode() { return element(); },
        replaceChildren(...children) { this.children = children; },
        getBoundingClientRect() { return { height: 160 }; },
        toDataURL() {
            if (captureError) throw new Error(captureError);
            return "data:image/png;base64,cGl4ZWxz";
        },
    });
    const elements = new Map(["status", "preview", "diagnostics", "stage"].map((id) => [id, element()]));
    elements.get("preview").hidden = true;
    const window = {
        matchMedia: () => ({ matches: false, dispatchEvent() {} }),
        addEventListener(_type, listener) { onMessage = listener; },
        parent: { postMessage(message) {
            if (message.method === "tools/call") {
                const args = message.params.arguments;
                published.push(args);
                if (args.png || args.error) resolveCapture(args);
            }
            if (message.id !== undefined) queueMicrotask(() => onMessage({
                source: window.parent,
                data: { jsonrpc: "2.0", id: message.id, result: {} },
            }));
        } },
    };
    Object.defineProperty(window, "devicePixelRatio", { get: () => 1, configurable: true });
    class MouseEvent {}
    for (const axis of ["offsetX", "offsetY"]) {
        Object.defineProperty(MouseEvent.prototype, axis, { get: () => 0, configurable: true });
    }
    class HTMLCanvasElement { getContext() {} }
    class HTMLImageElement {}
    Object.defineProperty(HTMLImageElement.prototype, "src", { get() {}, set() {}, configurable: true });
    const instance = { show: async () => {} };
    const interpreter = {
        default: async () => {}, run_event_loop() {},
        compile_from_string: async () => ({
            component: { create: async () => instance }, diagnostics: [],
        }),
    };
    const context = {
        window, document: {
            getElementById: (id) => elements.get(id),
            documentElement: { style: {} }, body: element(), createElement: element,
        },
        MouseEvent, HTMLCanvasElement, HTMLImageElement,
        ResizeObserver: class { observe() {} },
        console, URL, WebAssembly, Event,
        setTimeout, clearTimeout, queueMicrotask,
        requestAnimationFrame: (callback) => setTimeout(callback, 0),
        fetch: fetch ?? (async () => ({ json: async () => ({ version: "1.19.0" }) })),
        importInterpreter: async () => interpreter,
    };
    // The browser loads Wasm here; these tests exercise the view's surrounding lifecycle.
    const ready = runInNewContext(`(async () => {${script.replace("import(INTERPRETER_URL + tag)", "importInterpreter()")}})()`, context);
    return {
        ready, captured, published,
        async preview() {
            await ready;
            onMessage({ source: window.parent, data: {
                jsonrpc: "2.0", method: "ui/notifications/tool-input",
                params: { arguments: { source: "export component Preview {}" } },
            } });
            onMessage({ source: window.parent, data: {
                jsonrpc: "2.0", method: "ui/notifications/tool-result",
                params: { _meta: { "slint/previewId": "test-preview" } },
            } });
        },
    };
}

test("renders without waiting for version metadata", { timeout: 3000 }, async () => {
    const view = startView({ fetch: () => new Promise(() => {}) });
    await view.preview();
    const capture = await view.captured;
    assert.equal(capture.png, "cGl4ZWxz");
    assert.match(capture.runtime, /^Slint \(/);
});

test("renders when version metadata fails", { timeout: 3000 }, async () => {
    const view = startView({ fetch: async () => { throw new Error("offline"); } });
    await view.preview();
    assert.equal((await view.captured).png, "cGl4ZWxz");
});
