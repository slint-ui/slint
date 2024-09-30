// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as napi from "../rust-module.cjs";
export {
    Diagnostic,
    DiagnosticLevel,
    RgbaColor,
    Brush,
} from "../rust-module.cjs";

export { Model, ArrayModel, MapModel } from "./models";

import { Diagnostic } from "../rust-module.cjs";

/**
 *  Represents a two-dimensional point.
 */
export interface Point {
    /**
     * Defines the x coordinate of the point.
     */
    x: number;

    /**
     * Defines the y coordinate of the point.
     */
    y: number;
}

/**
 *  Represents a two-dimensional size.
 */
export interface Size {
    /**
     * Defines the width length of the size.
     */
    width: number;

    /**
     * Defines the height length of the size.
     */
    height: number;
}

/**
 * This type represents a window towards the windowing system, that's used to render the
 * scene of a component. It provides API to control windowing system specific aspects such
 * as the position on the screen.
 */
export interface Window {
    /** Gets or sets the logical position of the window on the screen. */
    logicalPosition: Point;

    /** Gets or sets the physical position of the window on the screen. */
    physicalPosition: Point;

    /** Gets or sets the logical size of the window on the screen, */
    logicalSize: Size;

    /** Gets or sets the physical size of the window on the screen, */
    physicalSize: Size;

    /** Gets or sets the window's fullscreen state **/
    fullscreen: boolean;

    /** Gets or sets the window's maximized state **/
    maximized: boolean;

    /** Gets or sets teh window's minimized state **/
    minimized: boolean;

    /**
     * Returns the visibility state of the window. This function can return false even if you previously called show()
     * on it, for example if the user minimized the window.
     */
    get visible(): boolean;

    /**
     * Shows the window on the screen. An additional strong reference on the
     * associated component is maintained while the window is visible.
     */
    show(): void;

    /** Hides the window, so that it is not visible anymore. */
    hide(): void;

    /** Issues a request to the windowing system to re-render the contents of the window. */
    requestRedraw(): void;
}

/**
 * An image data type that can be displayed by the Image element.
 *
 * This interface is inspired by the web [ImageData](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) interface.
 */
export interface ImageData {
    /**
     * Returns the path of the image, if it was loaded from disk. Otherwise
     * the property is undefined.
     */
    readonly path?: string;

    /**
     *  Returns the image as buffer.
     */
    get data(): Uint8Array;

    /**
     * Returns the width of the image in pixels.
     */
    get width(): number;

    /**
     *  Returns the height of the image in pixels.
     */
    get height(): number;
}

/**
 * This interface describes the public API of a Slint component that is common to all instances. Use this to
 * show() the window on the screen, access the window and subsequent window properties, or start the
 * Slint event loop with run().
 */
export interface ComponentHandle {
    /**
     * Shows the window and runs the event loop. The returned promise is resolved when the event loop
     * is terminated, for example when the last window was closed, or {@link quitEventLoop} was called.
     *
     * This function is a convenience for calling {@link show}, followed by {@link runEventLoop}, and
     * {@link hide} when the event loop's promise is resolved.
     */
    run(): Promise<unknown>;

    /**
     * Shows the component's window on the screen.
     */
    show();

    /**
     * Hides the component's window, so that it is not visible anymore.
     */
    hide();

    /**
     * Returns the {@link Window} associated with this component instance.
     * The window API can be used to control different aspects of the integration into the windowing system, such as the position on the screen.
     */
    get window(): Window;
}

/**
 * @hidden
 */
class Component implements ComponentHandle {
    #instance: napi.ComponentInstance;

    /**
     * @hidden
     */
    constructor(instance: napi.ComponentInstance) {
        this.#instance = instance;
    }

    get window(): Window {
        return this.#instance.window();
    }

    /**
     * @hidden
     */
    get component_instance(): napi.ComponentInstance {
        return this.#instance;
    }

    async run() {
        this.show();
        await runEventLoop();
        this.hide();
    }

    show() {
        this.#instance.window().show();
    }

    hide() {
        this.#instance.window().hide();
    }
}

/**
 * Represents an errors that can be emitted by the compiler.
 */
export class CompileError extends Error {
    /**
     * List of {@link Diagnostic} items emitted while compiling .slint code.
     */
    diagnostics: napi.Diagnostic[];

    /**
     * Creates a new CompileError.
     *
     * @param message human-readable description of the error.
     * @param diagnostics represent a list of diagnostic items emitted while compiling .slint code.
     */
    constructor(message: string, diagnostics: napi.Diagnostic[]) {
        const formattedDiagnostics = diagnostics
            .map(
                (d) =>
                    `[${d.fileName}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
            )
            .join("\n");

        let formattedMessage = message;
        if (diagnostics.length > 0) {
            formattedMessage += `\nDiagnostics:\n${formattedDiagnostics}`;
        }

        super(formattedMessage);
        this.diagnostics = diagnostics;
    }
}

/**
 * LoadFileOptions are used to defines different optional parameters that can be used to configure the compiler.
 */
export interface LoadFileOptions {
    /**
     * If set to true warnings from the compiler will not be printed to the console.
     */
    quiet?: boolean;

    /**
     * Sets the widget style the compiler is currently using when compiling .slint files.
     */
    style?: string;

    /**
     * Sets the include paths used for looking up `.slint` imports to the specified vector of paths.
     */
    includePaths?: Array<string>;

    /**
     * Sets library paths used for looking up `@library` imports to the specified map of library names to paths.
     */
    libraryPaths?: Record<string, string>;
}

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

    const slint_module = Object.create({});

    Object.keys(definitions).forEach((key) => {
        const definition = definitions[key];

        Object.defineProperty(
            slint_module,
            definition.name.replace(/-/g, "_"),
            {
                value: function (properties: any) {
                    const instance = definition.create();

                    if (instance == null) {
                        throw Error(
                            "Could not create a component handle for" +
                                filePath,
                        );
                    }

                    for (var key in properties) {
                        const value = properties[key];

                        if (value instanceof Function) {
                            instance.setCallback(key, value);
                        } else {
                            instance.setProperty(key, properties[key]);
                        }
                    }

                    const componentHandle = new Component(instance!);
                    instance!.definition().properties.forEach((prop) => {
                        const propName = prop.name.replace(/-/g, "_");

                        if (componentHandle[propName] !== undefined) {
                            console.warn(
                                "Duplicated property name " + propName,
                            );
                        } else {
                            Object.defineProperty(componentHandle, propName, {
                                get() {
                                    return instance!.getProperty(prop.name);
                                },
                                set(value) {
                                    instance!.setProperty(prop.name, value);
                                },
                                enumerable: true,
                            });
                        }
                    });

                    instance!.definition().callbacks.forEach((cb) => {
                        const callbackName = cb.replace(/-/g, "_");

                        if (componentHandle[callbackName] !== undefined) {
                            console.warn(
                                "Duplicated callback name " + callbackName,
                            );
                        } else {
                            Object.defineProperty(
                                componentHandle,
                                cb.replace(/-/g, "_"),
                                {
                                    get() {
                                        return function () {
                                            return instance!.invoke(
                                                cb,
                                                Array.from(arguments),
                                            );
                                        };
                                    },
                                    set(callback) {
                                        instance!.setCallback(cb, callback);
                                    },
                                    enumerable: true,
                                },
                            );
                        }
                    });

                    instance!.definition().functions.forEach((cb) => {
                        const functionName = cb.replace(/-/g, "_");

                        if (componentHandle[functionName] !== undefined) {
                            console.warn(
                                "Duplicated function name " + functionName,
                            );
                        } else {
                            Object.defineProperty(
                                componentHandle,
                                cb.replace(/-/g, "_"),
                                {
                                    get() {
                                        return function () {
                                            return instance!.invoke(
                                                cb,
                                                Array.from(arguments),
                                            );
                                        };
                                    },
                                    enumerable: true,
                                },
                            );
                        }
                    });

                    // globals
                    instance!.definition().globals.forEach((globalName) => {
                        if (componentHandle[globalName] !== undefined) {
                            console.warn(
                                "Duplicated property name " + globalName,
                            );
                        } else {
                            const globalObject = Object.create({});

                            instance!
                                .definition()
                                .globalProperties(globalName)
                                .forEach((prop) => {
                                    const propName = prop.name.replace(
                                        /-/g,
                                        "_",
                                    );

                                    if (globalObject[propName] !== undefined) {
                                        console.warn(
                                            "Duplicated property name " +
                                                propName +
                                                " on global " +
                                                global,
                                        );
                                    } else {
                                        Object.defineProperty(
                                            globalObject,
                                            propName,
                                            {
                                                get() {
                                                    return instance!.getGlobalProperty(
                                                        globalName,
                                                        prop.name,
                                                    );
                                                },
                                                set(value) {
                                                    instance!.setGlobalProperty(
                                                        globalName,
                                                        prop.name,
                                                        value,
                                                    );
                                                },
                                                enumerable: true,
                                            },
                                        );
                                    }
                                });

                            instance!
                                .definition()
                                .globalCallbacks(globalName)
                                .forEach((cb) => {
                                    const callbackName = cb.replace(/-/g, "_");

                                    if (
                                        globalObject[callbackName] !== undefined
                                    ) {
                                        console.warn(
                                            "Duplicated property name " +
                                                cb +
                                                " on global " +
                                                global,
                                        );
                                    } else {
                                        Object.defineProperty(
                                            globalObject,
                                            cb.replace(/-/g, "_"),
                                            {
                                                get() {
                                                    return function () {
                                                        return instance!.invokeGlobal(
                                                            globalName,
                                                            cb,
                                                            Array.from(
                                                                arguments,
                                                            ),
                                                        );
                                                    };
                                                },
                                                set(callback) {
                                                    instance!.setGlobalCallback(
                                                        globalName,
                                                        cb,
                                                        callback,
                                                    );
                                                },
                                                enumerable: true,
                                            },
                                        );
                                    }
                                });

                            instance!
                                .definition()
                                .globalFunctions(globalName)
                                .forEach((cb) => {
                                    const functionName = cb.replace(/-/g, "_");

                                    if (
                                        globalObject[functionName] !== undefined
                                    ) {
                                        console.warn(
                                            "Duplicated function name " +
                                                cb +
                                                " on global " +
                                                global,
                                        );
                                    } else {
                                        Object.defineProperty(
                                            globalObject,
                                            cb.replace(/-/g, "_"),
                                            {
                                                get() {
                                                    return function () {
                                                        return instance!.invokeGlobal(
                                                            globalName,
                                                            cb,
                                                            Array.from(
                                                                arguments,
                                                            ),
                                                        );
                                                    };
                                                },
                                                enumerable: true,
                                            },
                                        );
                                    }
                                });

                            Object.defineProperty(componentHandle, globalName, {
                                get() {
                                    return globalObject;
                                },
                                enumerable: true,
                            });
                        }
                    });

                    return Object.seal(componentHandle);
                },
            },
        );
    });
    return Object.seal(slint_module);
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
 * @param filePath The path to the file to load. Relative paths are resolved against the process' current working directory.
 * @param options An optional {@link LoadFileOptions} to configure additional Slint compilation settings,
 *                such as include search paths, library imports, or the widget style.
 * @returns Returns an object that is immutable and provides a constructor function for each exported Window component found in the `.slint` file.
 *          For instance, in the example above, a `Main` property is available, which can be used to create instances of the `Main` component using the `new` keyword.
 *          These instances offer properties and event handlers, adhering to the {@link ComponentHandle} interface.
 *          For further information on the available properties, refer to [Instantiating A Component](../index.html#md:instantiating-a-component).
 * @throws {@link CompileError} if errors occur during compilation.
 */
export function loadFile(filePath: string, options?: LoadFileOptions): Object {
    return loadSlint({
        fileData: { filePath, options },
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
 *          For further information on the available properties, refer to [Instantiating A Component](../index.html#md:instantiating-a-component).
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
    #terminateResolveFn: ((_value: unknown) => void) | null;

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
            napi.invokeFromEventLoop(() => {
                running_callback();
                running_callback = undefined;
            });
        }

        // Give the nodejs event loop 16 ms to tick. This polling is sub-optimal, but it's the best we
        // can do right now.
        const nodejsPollInterval = 16;
        const id = setInterval(() => {
            if (
                napi.processEvents() === napi.ProcessEventsResult.Exited ||
                this.#quit_loop
            ) {
                clearInterval(id);
                this.#terminateResolveFn!(undefined);
                this.#terminateResolveFn = null;
                this.#terminationPromise = null;
                return;
            }
        }, nodejsPollInterval);

        return this.#terminationPromise;
    }

    quit() {
        this.#quit_loop = true;
    }
}

var globalEventLoop: EventLoop = new EventLoop();

/**
 * Spins the Slint event loop and returns a promise that resolves when the loop terminates.
 *
 * If the event loop is already running, then this function returns the same promise as from
 * the earlier invocation.
 *
 * @param args As Function it defines a callback that's invoked once when the event loop is running.
 * @param args.runningCallback Optional callback that's invoked once when the event loop is running.
 *                         The function's return value is ignored.
 * @param args.quitOnLastWindowClosed if set to `true` event loop is quit after last window is closed otherwise
 *                          it is closed after {@link quitEventLoop} is called.
 *                          This is useful for system tray applications where the application needs to stay alive even if no windows are visible.
 *                          (default true).
 *
 * Note that the event loop integration with Node.js is slightly imperfect. Due to conflicting
 * implementation details between Slint's and Node.js' event loop, the two loops are merged
 * by spinning one after the other, at 16 millisecond intervals. This means that when the
 * application is idle, it continues to consume a low amount of CPU cycles, checking if either
 * event loop has any pending events.
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

/**
 * @hidden
 */
export namespace private_api {
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
        component.component_instance.sendMouseClick(x, y);
    }

    export function send_keyboard_string_sequence(
        component: Component,
        s: string,
    ) {
        component.component_instance.sendKeyboardStringSequence(s);
    }

    export import initTesting = napi.initTesting;
}
