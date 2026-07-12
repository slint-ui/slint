// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Test harness, loaded by runner.mjs into Chromium. Compiles one test case at
// a time with the wasm interpreter (testing backend), then runs the case's
// `js` code blocks the way the Node.js driver does, or — for cases without js
// blocks — instantiates the last exported component and checks its `test`
// property like the interpreter driver.

import * as slint from "/api/js/browser/dist/index.js";
// The import map redirects this to the pkg-testing build (testing backend).
import { compile_from_string_with_style } from "/api/js/browser/pkg/slint_wasm_interpreter";
import * as pkg from "/api/js/browser/pkg/slint_wasm_interpreter";
import { wrapModule } from "@slint-ui/common";

await slint.initWasm();

function fail(message) {
    const error = new Error(message);
    error.name = "AssertionError";
    throw error;
}

function looseDeepEqual(a, b) {
    // biome-ignore lint/suspicious/noDoubleEquals: node's assert.deepEqual is loose
    if (a == b) {
        return true;
    }
    if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) {
        return false;
    }
    return deepCompare(a, b, looseDeepEqual);
}

function strictDeepEqual(a, b) {
    if (Object.is(a, b)) {
        return true;
    }
    if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) {
        return false;
    }
    return deepCompare(a, b, strictDeepEqual);
}

function deepCompare(a, b, compare) {
    if (Array.isArray(a) !== Array.isArray(b)) {
        return false;
    }
    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
    if (keysA.length !== keysB.length) {
        return false;
    }
    return keysA.every((key) => compare(a[key], b[key]));
}

function show(value) {
    try {
        return JSON.stringify(value);
    } catch {
        return String(value);
    }
}

// The subset of node's `assert` module (strict-mode flavor of `assert()`
// itself, loose equal/deepEqual) that the test cases use.
const assert = Object.assign(
    (value, message) => {
        if (!value) {
            fail(message ?? `assertion failed: ${show(value)} is not truthy`);
        }
    },
    {
        ok: (value, message) => assert(value, message),
        equal: (actual, expected, message) => {
            // biome-ignore lint/suspicious/noDoubleEquals: node's assert.equal is loose
            if (!(actual == expected)) {
                fail(message ?? `${show(actual)} == ${show(expected)}`);
            }
        },
        notEqual: (actual, expected, message) => {
            // biome-ignore lint/suspicious/noDoubleEquals: node's assert.notEqual is loose
            if (actual == expected) {
                fail(message ?? `${show(actual)} != ${show(expected)}`);
            }
        },
        strictEqual: (actual, expected, message) => {
            if (!Object.is(actual, expected)) {
                fail(message ?? `${show(actual)} === ${show(expected)}`);
            }
        },
        deepEqual: (actual, expected, message) => {
            if (!looseDeepEqual(actual, expected)) {
                fail(message ?? `deepEqual: ${show(actual)} vs ${show(expected)}`);
            }
        },
        deepStrictEqual: (actual, expected, message) => {
            if (!strictDeepEqual(actual, expected)) {
                fail(message ?? `deepStrictEqual: ${show(actual)} vs ${show(expected)}`);
            }
        },
        throws: (fn, message) => {
            try {
                fn();
            } catch {
                return;
            }
            fail(message ?? "expected an exception");
        },
    },
);

// The API surface the js blocks reach through `slintlib`, mirroring what the
// Node.js driver provides via `require("slint-ui")`.
const slintlib = {
    Model: slint.Model,
    ArrayModel: slint.ArrayModel,
    MapModel: slint.MapModel,
    CompileError: slint.CompileError,
    Keys: pkg.Keys,
    StyledText: pkg.StyledText,
    private_api: {
        initTesting: () => {
            // The testing backend is installed at wasm module start.
        },
        mock_elapsed_time: (ms) => pkg.mockElapsedTime(ms),
        get_mocked_time: () => pkg.getMockedTime(),
        send_mouse_click: (component, x, y) =>
            component.component_instance.sendMouseClick(x, y),
        send_keyboard_string_sequence: (component, sequence) =>
            component.component_instance.sendKeyboardStringSequence(sequence),
        send_key_combo: (component, keys) =>
            component.component_instance.sendKeyCombo(keys),
    },
};

async function fetchText(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`could not load ${url}: HTTP ${response.status}`);
    }
    return await response.text();
}

async function compile(source, baseUrl) {
    const result = await compile_from_string_with_style(source, baseUrl, "", (path) =>
        fetchText(path),
    );
    for (const d of result.diagnostics) {
        if (d.level === 1) {
            console.warn(`${d.fileName}:${d.lineNumber}: ${d.message}`);
        }
    }
    if (result.error_string.length > 0) {
        throw new Error(`could not compile ${baseUrl}: ${result.error_string}`);
    }
    return result;
}

globalThis.runCase = async ({ baseUrl, source, js }) => {
    try {
        const result = await compile(source, baseUrl);
        const mod = wrapModule(result.definitions, result.structs, result.enums);

        if (js.length > 0) {
            for (const block of js) {
                // Sloppy mode on purpose: the Node.js driver runs the blocks as
                // CommonJS scripts, where e.g. assigning to a property the
                // sealed component doesn't have is silently ignored.
                new Function("slint", "slintlib", "assert", block)(mod, slintlib, assert);
            }
        } else {
            // No js blocks: instantiate the last exported component and check
            // its `test` property, like the interpreter driver.
            const definitions = Object.values(result.definitions);
            const definition = definitions[definitions.length - 1];
            if (definition !== undefined) {
                const instance = definition.create();
                if (definition.properties.some((p) => p.name === "test")) {
                    const value = instance.getProperty("test");
                    if (value !== true) {
                        throw new Error(`the 'test' property evaluated to ${show(value)}`);
                    }
                }
            }
        }
        return { ok: true };
    } catch (error) {
        return { ok: false, error: String(error?.stack ?? error) };
    }
};
