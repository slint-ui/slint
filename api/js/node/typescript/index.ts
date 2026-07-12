// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as napi from "../binding.cjs";
export {
    Diagnostic,
    DiagnosticLevel,
    RgbaColor,
    Brush,
    DataTransfer,
    StyledText,
    Keys,
} from "../binding.cjs";

export { language } from "./generated/language";

import { fileURLToPath } from "node:url";

import {
    CompileError,
    wrapModule,
    setModelBackend,
    setRunEventLoop,
    type Component,
    MapModel as _MapModel,
} from "@slint-ui/common";
import type { LoadFileOptions } from "@slint-ui/common";

export { CompileError, Component } from "@slint-ui/common";
export { Model, ArrayModel } from "@slint-ui/common";
export type {
    Point,
    Size,
    Window,
    ImageData,
    ComponentHandle,
    LoadFileOptions,
} from "@slint-ui/common";

// Initialize the model backend with napi functions
setModelBackend({
    createModelNotify: () => napi.jsModelNotifyNew(),
    notifyRowDataChanged: (handle: any, row: number) =>
        napi.jsModelNotifyRowDataChanged(handle, row),
    notifyRowAdded: (handle: any, row: number, count: number) =>
        napi.jsModelNotifyRowAdded(handle, row, count),
    notifyRowRemoved: (handle: any, row: number, count: number) =>
        napi.jsModelNotifyRowRemoved(handle, row, count),
    notifyReset: (handle: any) => napi.jsModelNotifyReset(handle),
});

type LoadData =
    | {
          fileData: {
              filePath: string;
              options?: LoadFileOptions;
          };
          from: "file";
      }
    | {
          fileData: {
              source: string;
              filePath: string;
              options?: LoadFileOptions;
          };
          from: "source";
      };

function loadSlint(loadData: LoadData): Object {
    const { filePath, options } = loadData.fileData;

    const compiler = new napi.ComponentCompiler();

    if (typeof options !== "undefined") {
        if (typeof options.style !== "undefined") {
            compiler.style = options.style;
        }
        if (typeof options.includePaths !== "undefined") {
            compiler.includePaths = options.includePaths;
        }
        if (typeof options.libraryPaths !== "undefined") {
            compiler.libraryPaths = options.libraryPaths;
        }
        if (typeof options.fileLoader !== "undefined") {
            compiler.fileLoader = options.fileLoader;
        }
    }

    const definitions =
        loadData.from === "file"
            ? compiler.buildFromPath(filePath)
            : compiler.buildFromSource(loadData.fileData.source, filePath);
    const diagnostics = compiler.diagnostics;

    if (diagnostics.length > 0) {
        const warnings = diagnostics.filter(
            (d) => d.level === napi.DiagnosticLevel.Warning,
        );

        if (typeof options !== "undefined" && options.quiet !== true) {
            warnings.forEach((w) => console.warn("Warning: " + w));
        }

        const errors = diagnostics.filter(
            (d) => d.level === napi.DiagnosticLevel.Error,
        );

        if (errors.length > 0) {
            throw new CompileError("Could not compile " + filePath, errors);
        }
    }

    return wrapModule(definitions, compiler.structs, compiler.enums);
}

/**
 * Loads the specified Slint file and returns an object containing functions to construct the exported
 * components defined within the Slint file.
 *
 * The following example loads a "Hello World" style Slint file and changes the Text label to a new greeting:
 * **`main.slint`**:
 * ```
 * export component Main inherits Window {
 *     in-out property <string> greeting <=> label.text;
 *     label := Text {
 *         text: "Hello World";
 *     }
 * }
 * ```
 *
 * **`index.js`**:
 * ```javascript
 * import * as slint from "slint-ui";
 * let ui = slint.loadFile("main.slint");
 * let main = new ui.Main();
 * main.greeting = "Hello friends";
 * ```
 *
 * @param filePath The path to the file to load as `string` or `URL`. Relative paths are resolved against the process' current working directory.
 * @param options An optional {@link LoadFileOptions} to configure additional Slint compilation settings,
 *                such as include search paths, library imports, or the widget style.
 * @returns Returns an object that is immutable and provides a constructor function for each exported Window component found in the `.slint` file.
 *          For instance, in the example above, a `Main` property is available, which can be used to create instances of the `Main` component using the `new` keyword.
 *          These instances offer properties and event handlers, adhering to the {@link ComponentHandle} interface.
 *          For further information on the available properties, refer to [Instantiating A Component](/#instantiating-a-component).
 * @throws {@link CompileError} if errors occur during compilation.
 */
export function loadFile(
    filePath: string | URL,
    options?: LoadFileOptions,
): Object {
    const pathname =
        filePath instanceof URL ? fileURLToPath(filePath) : filePath;
    return loadSlint({
        fileData: { filePath: pathname, options },
        from: "file",
    });
}

/**
 * Loads the given Slint source code and returns an object that contains a functions to construct the exported
 * components of the Slint source code.
 *
 * The following example loads a "Hello World" style Slint source code and changes the Text label to a new greeting:
 * ```js
 * import * as slint from "slint-ui";
 * const source = `export component Main {
 *      in-out property <string> greeting <=> label.text;
 *      label := Text {
 *          text: "Hello World";
 *      }
 * }`; // The content of main.slint
 *
 * let ui = slint.loadSource(source, "main.js");
 * let main = new ui.Main();
 * main.greeting = "Hello friends";
 * ```
 * @param source The Slint source code to load.
 * @param filePath A path to the file to show log and resolve relative import and images.
 *                 Relative paths are resolved against the process' current working directory.
 * @param options An optional {@link LoadFileOptions} to configure additional Slint compilation settings,
 *                such as include search paths, library imports, or the widget style.
 * @returns Returns an object that is immutable and provides a constructor function for each exported Window component found in the `.slint` file.
 *          For instance, in the example above, a `Main` property is available, which can be used to create instances of the `Main` component using the `new` keyword.
 *          These instances offer properties and event handlers, adhering to the {@link ComponentHandle} interface.
 *          For further information on the available properties, refer to [Instantiating A Component](/#instantiating-a-component).
 * @throws {@link CompileError} if errors occur during compilation.
 */
export function loadSource(
    source: string,
    filePath: string,
    options?: LoadFileOptions,
): Object {
    return loadSlint({
        fileData: { filePath, options, source },
        from: "source",
    });
}

class EventLoop {
    #quit_loop: boolean = false;
    #terminationPromise: Promise<unknown> | null = null;
    #terminateResolveFn: ((_value: unknown) => void) | null = null;

    start(
        running_callback?: Function,
        quitOnLastWindowClosed: boolean = true,
    ): Promise<unknown> {
        if (this.#terminationPromise != null) {
            return this.#terminationPromise;
        }

        this.#terminationPromise = new Promise((resolve) => {
            this.#terminateResolveFn = resolve;
        });
        this.#quit_loop = false;

        napi.setQuitOnLastWindowClosed(quitOnLastWindowClosed);

        if (running_callback !== undefined) {
            const cb = running_callback;
            napi.invokeFromEventLoop(() => {
                cb();
                running_callback = undefined;
            });
        }

        if (napi.hasIntegratedEventLoop()) {
            try {
                // Register a uv_prepare handle that pumps Slint events
                // on every libuv iteration.  The callback fires when the
                // Slint event loop terminates.
                napi.startIntegratedEventLoop(() => this.#resolve());
                return this.#terminationPromise;
            } catch {
                // process_events not supported (e.g. testing backend) —
                // fall through to the polling fallback.
            }
        }

        // Fallback for Windows, Deno, and runtimes without uv_backend_fd().
        {
            const nodejsPollInterval = 16;
            const id = setInterval(() => {
                if (
                    napi.processEvents() === napi.ProcessEventsResult.Exited ||
                    this.#quit_loop
                ) {
                    clearInterval(id);
                    this.#resolve();
                    return;
                }
            }, nodejsPollInterval);
        }

        return this.#terminationPromise;
    }

    #resolve() {
        if (this.#terminateResolveFn === null) {
            return;
        }
        this.#terminateResolveFn(undefined);
        this.#terminateResolveFn = null;
        this.#terminationPromise = null;
    }

    quit() {
        this.#quit_loop = true;
        napi.quitEventLoop();
    }
}

const globalEventLoop: EventLoop = new EventLoop();

/**
 * Spins the Slint event loop and returns a promise that resolves when the loop terminates.
 *
 * If the event loop is already running, then this function returns the same promise as from
 * the earlier invocation.
 *
 * @param args As Function it defines a callback that's invoked once when the event loop is running.
 * @param args.runningCallback Optional callback that's invoked once when the event loop is running.
 *                         The function's return value is ignored.
 * @param args.quitOnLastWindowClosed if set to `true` the loop quits once the last window is closed
 *                          and the last visible system tray icon is hidden; otherwise it runs until
 *                          {@link quitEventLoop} is called. A visible SystemTrayIcon keeps the loop alive
 *                          on its own under the default, so set this to `false` only when an
 *                          application must run without any visible UI. (default true).
 *
 * On Linux and macOS with Node.js,
 * Slint uses an efficient event loop integration that watches libuv's backend
 * file descriptor from a background thread.
 * This provides zero idle CPU usage and near-instant response to both UI and
 * JavaScript events.
 *
 * On Windows and other runtimes (Deno),
 * the integration falls back to polling at 16 millisecond intervals,
 * which consumes a small amount of CPU when idle.
 */
export function runEventLoop(
    args?:
        | Function
        | { runningCallback?: Function; quitOnLastWindowClosed?: boolean },
): Promise<unknown> {
    if (args === undefined) {
        return globalEventLoop.start(undefined);
    }

    if (args instanceof Function) {
        return globalEventLoop.start(args);
    }

    return globalEventLoop.start(
        args.runningCallback,
        args.quitOnLastWindowClosed,
    );
}

/**
 * Stops a spinning event loop. This function returns immediately, and the promise returned
 from run_event_loop() will resolve in a later tick of the nodejs event loop.
 */
export function quitEventLoop() {
    globalEventLoop.quit();
}

// Wire up Component.run() to use our event loop
setRunEventLoop(() => runEventLoop());

/**
 * Initialize translations.
 *
 * Call this with the path where translations are located. This function internally calls the [bindtextdomain](https://man7.org/linux/man-pages/man3/bindtextdomain.3.html) function from gettext.
 *
 * Translations are expected to be found at <path>/<locale>/LC_MESSAGES/<domain>.mo, where path is the directory passed as an argument to this function, locale is a locale name (e.g., en, en_GB, fr), and domain is the package name.
 *
 * @param domain defines the domain name e.g. name of the package.
 * @param path specifies the directory as `string` or as `URL` in which gettext should search for translations.
 *
 * For example, assuming this is in a package called example and the default locale is configured to be French, it will load translations at runtime from ``/path/to/example/translations/fr/LC_MESSAGES/example.mo`.
 *
 * ```js
 * import * as slint from "slint-ui";
 * slint.initTranslations("example", new URL("translations/", import.meta.url));
 * ````
 */
export function initTranslations(domain: string, path: string | URL) {
    const pathname = path instanceof URL ? fileURLToPath(path) : path;
    napi.initTranslations(domain, pathname);
}

/**
 * Sets the application id for use on Wayland or X11 with [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
 * compliant window managers. This must be set before the window is shown.
 */
export function setXdgAppId(app_id: string) {
    napi.setXdgAppId(app_id);
}

/**
 * @hidden
 */
export namespace private_api {
    /**
     * Provides rows that are generated by a map function based on the rows of another Model.
     *
     * @template T item type of source model that is mapped to U.
     * @template U the type of the mapped items
     *
     * ## Example
     *
     *  Here we have a {@link ArrayModel} holding rows of a custom interface `Name` and a {@link MapModel} that maps the name rows
     *  to single string rows.
     *
     * ```ts
     * import { Model, ArrayModel, MapModel } from "./index";
     *
     * interface Name {
     *     first: string;
     *     last: string;
     * }
     *
     * const model = new ArrayModel<Name>([
     *     {
     *         first: "Hans",
     *         last: "Emil",
     *     },
     *     {
     *         first: "Max",
     *         last: "Mustermann",
     *     },
     *     {
     *         first: "Roman",
     *         last: "Tisch",
     *     },
     * ]);
     *
     * const mappedModel = new MapModel(
     *     model,
     *     (data) => {
     *         return data.last + ", " + data.first;
     *     }
     * );
     *
     * // prints "Emil, Hans"
     * console.log(mappedModel.rowData(0));
     *
     * // prints "Mustermann, Max"
     * console.log(mappedModel.rowData(1));
     *
     * // prints "Tisch, Roman"
     * console.log(mappedModel.rowData(2));
     *
     * // Alternatively you can use the shortcut {@link MapModel.map}.
     *
     * const model = new ArrayModel<Name>([
     *     {
     *         first: "Hans",
     *         last: "Emil",
     *     },
     *     {
     *         first: "Max",
     *         last: "Mustermann",
     *     },
     *     {
     *         first: "Roman",
     *         last: "Tisch",
     *     },
     * ]);
     *
     * const mappedModel = model.map(
     *     (data) => {
     *         return data.last + ", " + data.first;
     *     }
     * );
     *
     *
     * // prints "Emil, Hans"
     * console.log(mappedModel.rowData(0));
     *
     * // prints "Mustermann, Max"
     * console.log(mappedModel.rowData(1));
     *
     * // prints "Tisch, Roman"
     * console.log(mappedModel.rowData(2));
     *
     * // You can modifying the underlying {@link ArrayModel}:
     *
     * const model = new ArrayModel<Name>([
     *     {
     *         first: "Hans",
     *         last: "Emil",
     *     },
     *     {
     *         first: "Max",
     *         last: "Mustermann",
     *     },
     *     {
     *         first: "Roman",
     *         last: "Tisch",
     *     },
     * ]);
     *
     * const mappedModel = model.map(
     *     (data) => {
     *         return data.last + ", " + data.first;
     *     }
     * );
     *
     * model.setRowData(1, { first: "Minnie", last: "Musterfrau" } );
     *
     * // prints "Emil, Hans"
     * console.log(mappedModel.rowData(0));
     *
     * // prints "Musterfrau, Minnie"
     * console.log(mappedModel.rowData(1));
     *
     * // prints "Tisch, Roman"
     * console.log(mappedModel.rowData(2));
     * ```
     */
    export const MapModel = _MapModel;

    export import mock_elapsed_time = napi.mockElapsedTime;
    export import get_mocked_time = napi.getMockedTime;
    export import ComponentCompiler = napi.ComponentCompiler;
    export import ComponentDefinition = napi.ComponentDefinition;
    export import ComponentInstance = napi.ComponentInstance;
    export import ValueType = napi.ValueType;
    export import Window = napi.Window;

    export import SlintBrush = napi.SlintBrush;
    export import SlintRgbaColor = napi.SlintRgbaColor;
    export import SlintSize = napi.SlintSize;
    export import SlintPoint = napi.SlintPoint;
    export import SlintImageData = napi.SlintImageData;

    export function send_mouse_click(
        component: Component,
        x: number,
        y: number,
    ) {
        (component.component_instance as napi.ComponentInstance).sendMouseClick(
            x,
            y,
        );
    }

    export function send_keyboard_string_sequence(
        component: Component,
        s: string,
    ) {
        (
            component.component_instance as napi.ComponentInstance
        ).sendKeyboardStringSequence(s);
    }

    export function send_key_combo(component: Component, keys: string[]) {
        (component.component_instance as napi.ComponentInstance).sendKeyCombo(
            keys,
        );
    }

    export import initTesting = napi.initTesting;

    /**
     * Returns the optional capabilities that were compiled into the loaded
     * native binary, e.g. `"testing"`, `"system-testing"` and `"mcp"`. When the
     * default binary is loaded this is empty; when the "dev" binary is loaded
     * it contains the additional features. See binding.cjs.
     */
    export import buildFeatures = napi.buildFeatures;
}
