// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const html = readFileSync(new URL("../view.html.in", import.meta.url), "utf8");
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

function startView({
    fetch,
    captureError,
    compile,
    toolCallError,
    missingImage,
    initialize = async () => {},
    callTool = () => ({}),
} = {}) {
    const captures = [];
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
        addEventListener() {},
        setAttribute() {},
        hidePopover() {},
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
        replaceWith() {},
        replaceChildren(...children) {
            this.children = children;
        },
        toDataURL() {
            if (captureError) throw new Error(captureError);
            return "data:image/png;base64,cGl4ZWxz";
        },
    });
    const elements = new Map(
        ["status", "preview", "diagnostics", "stage", "crash", "crash-details", "retry", "preview-frame", "annotate",
            "annotation-layer", "annotation-snapshot", "annotation-pin",
            "annotation-editor", "annotation-comment", "annotation-send", "annotation-error",
            "preview-tools", "preview-light", "preview-dark"].map((id) => [
            id,
            element(),
        ]),
    );
    elements.get("preview").hidden = true;
    elements.get("crash").hidden = true;
    const window = {
        matchMedia: () => ({ matches: false, dispatchEvent() {} }),
        addEventListener(_type, listener) {
            onMessage = listener;
        },
        parent: {
            postMessage(message) {
                if (message.method === "tools/call") {
                    const args = message.params.arguments;
                    if (args.png || args.error) {
                        captures.push(args);
                        resolveCapture(args);
                    }
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
                                ...(toolCallError &&
                                message.method === "tools/call"
                                    ? { error: toolCallError }
                                    : {
                                          result:
                                              message.method === "tools/call"
                                                  ? message.params.name === "wait_preview_update"
                                                      ? new Promise(() => {})
                                                      : callTool(message.params)
                                                  : {},
                                      }),
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
    const instance = {
        show: async () => {
            if (missingImage) {
                new HTMLImageElement().src = "slint-file:///missing.png";
            }
        },
    };
    const interpreter = {
        default: initialize,
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
            addEventListener() {},
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
        console: { error() {} },
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
        captures,
        reported,
        status: elements.get("status"),
        diagnostics: elements.get("diagnostics"),
        element: (id) => elements.get(id),
        retry: () => elements.get("retry").onclick(),
        panic() {
            context.console.error(
                "panicked at test.rs:1:1:\ntest panic\nhttps://example.com/interpreter.js#instance-1:1:1",
            );
        },
        async preview(args = { source: "export component Preview {}" }) {
            await ready;
            onMessage({
                source: window.parent,
                data: {
                    jsonrpc: "2.0",
                    method: "ui/notifications/tool-input",
                    params: {
                        arguments: args,
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

test("initializes the interpreter while the source file is still loading", {
    timeout: 3000,
}, async () => {
    let finishRead, initialized;
    const source = new Promise((resolve) => { finishRead = resolve; });
    const initialization = new Promise((resolve) => { initialized = resolve; });
    const view = startView({
        initialize: async () => initialized(),
        callTool: ({ name }) => name === "read_slint_file" ? source : {},
    });
    await view.preview({ path: "/project/main.slint" });
    await initialization;
    assert.equal(view.captures.length, 0);
    finishRead({ content: [{ type: "text", text: "export component Preview {}" }] });
    assert.equal((await view.captured).png, "cGl4ZWxz");
});

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

test("hides compiler warnings and notes while reporting warnings to the model", {
    timeout: 3000,
}, async () => {
    const view = startView({
        compile: async () => ({
            component: { create: async () => ({ show: async () => {} }) },
            diagnostics: [
                { level: 1, message: "test warning", lineNumber: 1, columnNumber: 2 },
                { level: 2, message: "test note", lineNumber: 3, columnNumber: 4 },
            ],
        }),
    });
    await view.preview();
    assert.equal((await view.captured).png, "cGl4ZWxz");
    assert.equal(view.diagnostics.children.length, 0);
    assert.ok((await view.reported).includes("main.slint:1:2: warning: test warning"));
});

test("shows compiler errors without warnings and notes", {
    timeout: 3000,
}, async () => {
    const view = startView({
        compile: async () => ({
            component: null,
            diagnostics: [
                { level: 0, message: "test error", lineNumber: 1, columnNumber: 2 },
                { level: 1, message: "test warning", lineNumber: 3, columnNumber: 4 },
                { level: 2, message: "test note", lineNumber: 5, columnNumber: 6 },
            ],
        }),
    });
    await view.preview();
    const capture = await view.captured;
    assert.equal(capture.error, "main.slint:1:2: error: test error");
    assert.equal(view.diagnostics.children.length, 1);
    assert.equal(view.diagnostics.children[0].className, "error");
    assert.equal(view.diagnostics.children[0].textContent, capture.error);
    assert.ok((await view.reported).includes("main.slint:3:4: warning: test warning"));
});

test("reports missing images to the model without displaying warnings", {
    timeout: 3000,
}, async () => {
    const view = startView({ missingImage: true });
    await view.preview();
    assert.equal((await view.captured).png, "cGl4ZWxz");
    assert.equal(view.diagnostics.children.length, 0);
    assert.equal(
        await view.reported,
        "The Slint preview couldn't load image /missing.png: no file data",
    );
});

test("retry replaces a late crash with a new preview and clears the panel", {
    timeout: 3000,
}, async () => {
    const calls = [];
    const view = startView({
        callTool({ name, arguments: args }) {
            if (name === "preview_slint") {
                calls.push(args.source);
                return { _meta: { "slint/previewId": "retry-preview" } };
            }
            return {};
        },
    });
    await view.preview();
    await view.captured;
    view.element("retry").disabled = true;
    view.panic();
    const failure = view.captures.at(-1);
    assert.ok(failure.error.includes("test panic"));
    assert.equal(failure.png, undefined);
    assert.equal(failure.runtime, "Slint 1.19.0 (snapshot)");
    assert.equal(view.element("crash").hidden, false);
    assert.equal(view.element("retry").disabled, false);
    assert.ok(view.element("crash-details").textContent.includes("test panic"));
    assert.equal(view.element("status").textContent, "");
    await view.retry();
    assert.deepEqual(calls, ["export component Preview {}"]);
    assert.equal(view.captures.at(-1).previewId, "retry-preview");
    assert.equal(view.captures.at(-1).png, "cGl4ZWxz");
    assert.equal(view.element("crash").hidden, true);
    assert.equal(view.element("retry").disabled, false);
});

test("a failed retry leaves the crash panel available", {
    timeout: 3000,
}, async () => {
    const view = startView({
        callTool: ({ name }) =>
            name === "preview_slint"
                ? {
                      isError: true,
                      content: [{ type: "text", text: "connection lost" }],
                  }
                : {},
    });
    await view.preview();
    await view.captured;
    view.panic();
    await view.retry();
    assert.equal(view.element("crash").hidden, false);
    assert.ok(view.element("status").textContent.includes("connection lost"));
    assert.equal(view.element("retry").disabled, false);
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
            "Couldn't capture the preview: canvas unavailable",
        ),
    );
    assert.equal(capture.png, undefined);
});

test("shows the message of a refused tool call", {
    timeout: 3000,
}, async () => {
    const view = startView({
        toolCallError: {
            code: -32000,
            message: "Tool 'publish_preview_capture' is not permitted",
        },
    });
    await view.preview();
    while (!view.status.textContent.startsWith("Couldn't capture")) {
        await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.equal(
        view.status.textContent,
        "Couldn't capture the preview: Tool 'publish_preview_capture' is not permitted",
    );
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
