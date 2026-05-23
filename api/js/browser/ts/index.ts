// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import __wbg_init, {
    compile_from_string_with_style,
    quit_event_loop as wasmQuitEventLoop,
    run_event_loop as wasmRunEventLoop,
    set_next_canvas_id,
    wasm_model_notify_new,
    wasm_model_notify_row_data_changed,
    wasm_model_notify_row_added,
    wasm_model_notify_row_removed,
    wasm_model_notify_reset,
} from "../pkg/slint_wasm_interpreter";
import type {
    CompilationResult,
    WasmSharedModelNotify,
    WrappedDefinition,
} from "../pkg/slint_wasm_interpreter";

import {
    CompileError,
    wrapModule,
    setModelBackend,
    setRunEventLoop,
} from "slint-js-common";
import type {
    Diagnostic,
    DefinitionLike,
} from "slint-js-common";

export { CompileError, Component } from "slint-js-common";
export { Model, ArrayModel, MapModel } from "slint-js-common";
export type {
    Point,
    Size,
    Window,
    ImageData,
    ComponentHandle,
} from "slint-js-common";

/**
 * Options for compiling a Slint source string.
 */
export interface CompileOptions {
    /** Widget style: "fluent", "material", "cosmic", etc. */
    style?: string;
    /** Whether to silence warnings printed to the console. */
    quiet?: boolean;
    /**
     * Callback invoked when the compiler needs to resolve an `import` —
     * given a URL, returns the imported source.
     */
    fileLoader?: (url: string) => Promise<string>;
}

let _wasmInitPromise: Promise<unknown> | null = null;

/**
 * Lazily instantiate the underlying WebAssembly module.
 *
 * Called automatically by {@link loadSource}; can also be called explicitly
 * to pre-warm the module (e.g. during page load) before any compilation.
 */
export function initWasm(): Promise<unknown> {
    if (_wasmInitPromise === null) {
        _wasmInitPromise = __wbg_init().then((out) => {
            // Wire model notifications now that the wasm module is loaded.
            // Without this, `new ArrayModel(...)` would throw.
            setModelBackend({
                createModelNotify: () => wasm_model_notify_new(),
                notifyRowDataChanged: (h, row) =>
                    wasm_model_notify_row_data_changed(
                        h as WasmSharedModelNotify,
                        row,
                    ),
                notifyRowAdded: (h, row, count) =>
                    wasm_model_notify_row_added(
                        h as WasmSharedModelNotify,
                        row,
                        count,
                    ),
                notifyRowRemoved: (h, row, count) =>
                    wasm_model_notify_row_removed(
                        h as WasmSharedModelNotify,
                        row,
                        count,
                    ),
                notifyReset: (h) =>
                    wasm_model_notify_reset(h as WasmSharedModelNotify),
            });
            return out;
        });
    }
    return _wasmInitPromise;
}

/**
 * Compile a Slint source string and return a sealed module object containing
 * the exported components, structs and enums.
 *
 * @param source the `.slint` source code
 * @param baseUrl a URL used as the base for resolving `import`s in the source
 * @param opts optional compilation options
 *
 * ### Example
 * ```js
 * import * as slint from "slint-ui-browser";
 *
 * const ui = await slint.loadSource(`
 *     export component App inherits Window {
 *         in-out property <string> greeting: "Hello";
 *     }
 * `, "app.slint");
 *
 * const app = new ui.App();
 * app.show();
 * ```
 */
export async function loadSource(
    source: string,
    baseUrl = "source.slint",
    opts: CompileOptions = {},
): Promise<Record<string, unknown>> {
    await initWasm();

    const result: CompilationResult = await compile_from_string_with_style(
        source,
        baseUrl,
        opts.style ?? "",
        opts.fileLoader,
    );

    const diagnostics = result.diagnostics as Diagnostic[];
    if (diagnostics.length > 0 && !opts.quiet) {
        for (const d of diagnostics) {
            if (d.level === 1) {
                console.warn(formatDiagnostic(d));
            }
        }
    }

    if (result.error_string.length > 0) {
        const errors = diagnostics.filter((d) => d.level === 0);
        throw new CompileError(
            `Could not compile ${baseUrl}`,
            errors,
        );
    }

    const definitions = result.definitions as Record<string, WrappedDefinition>;
    return wrapModule(
        definitions as unknown as Record<string, DefinitionLike>,
        result.structs as Record<string, unknown>,
        result.enums as Record<string, unknown>,
    ) as Record<string, unknown>;
}

function formatDiagnostic(d: Diagnostic): string {
    const where = d.fileName
        ? `${d.fileName}:${d.lineNumber}:${d.columnNumber}`
        : `${d.lineNumber}:${d.columnNumber}`;
    return `Warning: ${where}: ${d.message}`;
}

/**
 * Set the HTML canvas element ID that the next created component will render
 * into. The DOM element with this id (a `<canvas>`) is consumed by the next
 * `new SomeComponent()` call.
 */
export function setCanvasId(id: string): void {
    set_next_canvas_id(id);
}

let _quitResolve: (() => void) | null = null;
let _eventLoopPromise: Promise<void> | null = null;

/**
 * Returns a Promise that resolves when {@link quitEventLoop} is called.
 *
 * Unlike `slint-ui` on Node.js — where the loop ends when the last window
 * closes — a browser page is the application, so the loop must be ended
 * explicitly. Closing the canvas does not end the loop.
 */
export function runEventLoop(): Promise<void> {
    if (_eventLoopPromise === null) {
        _eventLoopPromise = new Promise<void>((resolve) => {
            _quitResolve = resolve;
        });
        // Spawn the winit event loop. `with_spawn_event_loop(true)` on the
        // backend doesn't actually start the loop on its own; the
        // slint_interpreter call below does. It throws on second invocation
        // (winit can't be re-entered) which is fine — the loop is already up.
        try {
            wasmRunEventLoop();
        } catch (err) {
            console.warn("[slint-wasm] run_event_loop:", err);
        }
    }
    return _eventLoopPromise;
}

/**
 * Stops the winit-side event loop and resolves any pending Promise returned
 * from {@link runEventLoop}. After this, the canvas no longer repaints or
 * processes input.
 */
export function quitEventLoop(): void {
    wasmQuitEventLoop();
    if (_quitResolve !== null) {
        const resolve = _quitResolve;
        _quitResolve = null;
        _eventLoopPromise = null;
        resolve();
    }
}

setRunEventLoop(() => runEventLoop());
