// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const html = readFileSync(new URL("../view.html", import.meta.url), "utf8");
const scripts = html.split('<script type="module">');
assert.equal(scripts.length, 2);
const script = scripts[1]
    .split("</script>")[0]
    .replace("__INTERPRETER_URL__", "https://example.com/interpreter.js")
    .replace("__CHANNEL__", "snapshot");
const importParts = script.split("import(INTERPRETER_URL + tag)");
assert.equal(
    importParts.length,
    2,
    "the view must have one interpreter import",
);
const testScript = importParts.join("importInterpreter()");

function startView({ fetch, captureError, compile } = {}) {
    let onMessage;
    let resolveCapture;
    const captured = new Promise((resolve) => {
        resolveCapture = resolve;
    });
    let resolveReport;
    const reported = new Promise((resolve) => {
        resolveReport = resolve;
    });
    const element = () => ({
        style: { setProperty() {} },
        dataset: {},
        classList: { add() {}, remove() {} },
        textContent: "",
        className: "",
        children: [],
        hidden: false,
        width: 320,
        height: 160,
        offsetWidth: 320,
        offsetHeight: 160,
        clientWidth: 320,
        cloneNode() {
            return element();
        },
        replaceChildren(...children) {
            this.children = children;
        },
        toDataURL() {
            if (captureError) throw new Error(captureError);
            return "data:image/png;base64,cGl4ZWxz";
        },
    });
    const elements = new Map(
        ["status", "preview", "diagnostics", "stage"].map((id) => [
            id,
            element(),
        ]),
    );
    elements.get("preview").hidden = true;
    const window = {
        matchMedia: () => ({ matches: false, dispatchEvent() {} }),
        addEventListener(_type, listener) {
            onMessage = listener;
        },
        parent: {
            postMessage(message) {
                if (message.method === "tools/call") {
                    const args = message.params.arguments;
                    if (args.png || args.error) resolveCapture(args);
                }
                if (message.method === "ui/update-model-context") {
                    resolveReport(message.params.content[0].text);
                }
                if (message.id !== undefined)
                    queueMicrotask(() =>
                        onMessage({
                            source: window.parent,
                            data: {
                                jsonrpc: "2.0",
                                id: message.id,
                                result: {},
                            },
                        }),
                    );
            },
        },
    };
    Object.defineProperty(window, "devicePixelRatio", {
        get: () => 1,
        configurable: true,
    });
    class MouseEvent {}
    for (const axis of ["offsetX", "offsetY"]) {
        Object.defineProperty(MouseEvent.prototype, axis, {
            get: () => 0,
            configurable: true,
        });
    }
    class HTMLCanvasElement {
        getContext() {}
    }
    class HTMLImageElement {}
    Object.defineProperty(HTMLImageElement.prototype, "src", {
        get() {},
        set() {},
        configurable: true,
    });
    const instance = { show: async () => {} };
    const interpreter = {
        default: async () => {},
        run_event_loop() {},
        compile_from_string:
            compile ??
            (async () => ({
                component: { create: async () => instance },
                diagnostics: [],
            })),
    };
    const context = {
        window,
        document: {
            getElementById: (id) => elements.get(id),
            documentElement: { style: {} },
            body: element(),
            createElement: element,
        },
        MouseEvent,
        HTMLCanvasElement,
        HTMLImageElement,
        ResizeObserver: class {
            observe() {}
        },
        console,
        URL,
        WebAssembly,
        Event,
        setTimeout,
        clearTimeout,
        queueMicrotask,
        requestAnimationFrame: (callback) => setTimeout(callback, 0),
        fetch:
            fetch ??
            (async () => ({ json: async () => ({ version: "1.19.0" }) })),
        importInterpreter: async () => interpreter,
    };
    // The browser loads Wasm here; these tests exercise the view's surrounding lifecycle.
    const ready = runInNewContext(`(async () => {${testScript}})()`, context);
    return {
        captured,
        reported,
        async preview() {
            await ready;
            onMessage({
                source: window.parent,
                data: {
                    jsonrpc: "2.0",
                    method: "ui/notifications/tool-input",
                    params: {
                        arguments: { source: "export component Preview {}" },
                    },
                },
            });
            onMessage({
                source: window.parent,
                data: {
                    jsonrpc: "2.0",
                    method: "ui/notifications/tool-result",
                    params: { _meta: { "slint/previewId": "test-preview" } },
                },
            });
        },
    };
}

test("renders without waiting for version metadata", {
    timeout: 3000,
}, async () => {
    const view = startView({ fetch: () => new Promise(() => {}) });
    await view.preview();
    const capture = await view.captured;
    assert.equal(capture.png, "cGl4ZWxz");
    assert.ok(capture.runtime.startsWith("Slint ("));
});

test("renders when version metadata fails", { timeout: 3000 }, async () => {
    const view = startView({
        fetch: async () => {
            throw new Error("offline");
        },
    });
    await view.preview();
    assert.equal((await view.captured).png, "cGl4ZWxz");
});

test("publishes a terminal error when canvas capture fails", {
    timeout: 3000,
}, async () => {
    const view = startView({ captureError: "canvas unavailable" });
    await view.preview();
    const capture = await view.captured;
    assert.equal(capture.previewId, "test-preview");
    assert.ok(
        capture.error.includes(
            "Couldn't capture the preview: Error: canvas unavailable",
        ),
    );
    assert.equal(capture.png, undefined);
});

test("diagnostics use version metadata that arrives during compilation", {
    timeout: 3000,
}, async () => {
    let finishMetadata;
    const metadata = new Promise((resolve) => {
        finishMetadata = resolve;
    });
    const view = startView({
        fetch: async () => ({ json: () => metadata }),
        compile: async () => {
            finishMetadata({ version: "1.19.0" });
            await new Promise((resolve) => setTimeout(resolve, 0));
            return {
                component: null,
                diagnostics: [
                    {
                        level: 0,
                        message: "test diagnostic",
                        lineNumber: 1,
                        columnNumber: 1,
                    },
                ],
            };
        },
    });
    await view.preview();
    assert.ok(
        (await view.reported).includes(
            "with Slint 1.19.0 (snapshot), reported:",
        ),
    );
    assert.equal((await view.captured).runtime, "Slint 1.19.0 (snapshot)");
});
