// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const view = readFileSync(new URL("../view.html", import.meta.url), "utf8");
const renderSource = view.match(/^  async function render\([\s\S]*?^  }/m)[0];
const reportSource = view.match(/^  function reportToModel\([\s\S]*?^  }/m)[0];

function preview(results) {
    const messages = [];
    const canvas = { hidden: true };
    const diagnosticsEl = {
        children: [],
        replaceChildren(...children) {
            this.children = children;
        },
    };
    const instance = {
        shown: 0,
        show() {
            this.shown++;
        },
    };
    const component = {
        create: async () => instance,
        create_with_existing_window: async (previous) => previous,
    };
    const interpreter = {
        runtime: Promise.resolve("Slint 1.19.0 (snapshot)"),
        slint: Promise.resolve({
            compile_from_string: async () => {
                const { diagnostics = [], valid = true } = results.shift();
                return { diagnostics, component: valid ? component : null };
            },
        }),
    };
    const render = runInNewContext(
        `let instance = null; let runtime = null;
        ${renderSource}
        ${reportSource}
        render`,
        {
            canvas,
            diagnosticsEl,
            LEVELS: ["error", "warning", "note"],
            FILE_SCHEME: "slint-file:",
            document: { createElement: () => ({}) },
            loadInterpreter: () => interpreter,
            untilPanic: async (_interpreter, promise) => promise,
            showStatus() {},
            startMeasuring() {},
            finishMeasuring: async () => {},
            request: async (method, params) =>
                messages.push({ method, params }),
        },
    );
    return { render, messages, canvas, diagnosticsEl, instance };
}

function diagnostic(level, message) {
    return {
        level,
        message,
        fileName: "main.slint",
        lineNumber: 3,
        columnNumber: 5,
    };
}

test("a clean compile renders without updating model context", async () => {
    const state = preview([{}, {}]);
    await state.render({ source: "first" });
    await state.render({ source: "second" });
    assert.equal(state.messages.length, 0);
    assert.equal(state.instance.shown, 2);
    assert.equal(state.canvas.hidden, false);
});

for (const [level, label, message] of [
    [0, "error", "Unknown property colour"],
    [1, "warning", "Deprecated property"],
]) {
    test(`a compile ${label} still updates model context`, async () => {
        const state = preview([
            { diagnostics: [diagnostic(level, message)], valid: level !== 0 },
        ]);
        await state.render({ source: "test" });
        assert.equal(state.messages.length, 1);
        assert.equal(state.messages[0].method, "ui/update-model-context");
        assert.equal(
            state.messages[0].params.content[0].text,
            `The Slint preview, with Slint 1.19.0 (snapshot), reported:\nmain.slint:3:5: ${label}: ${message}`,
        );
        assert.equal(state.diagnosticsEl.children[0].className, label);
        assert.equal(state.instance.shown, level === 0 ? 0 : 1);
    });
}

test("recovery clears visible diagnostics without a success context update", async () => {
    const state = preview([
        {
            diagnostics: [diagnostic(0, "Unknown property colour")],
            valid: false,
        },
        {},
    ]);
    await state.render({ source: "invalid" });
    assert.equal(state.messages.length, 1);
    assert.equal(state.canvas.hidden, true);
    await state.render({ source: "corrected" });
    assert.equal(state.messages.length, 1);
    assert.equal(state.diagnosticsEl.children.length, 0);
    assert.equal(state.instance.shown, 1);
    assert.equal(state.canvas.hidden, false);
});
