// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import * as napi from "./rust-module";
export {
    Diagnostic,
    DiagnosticLevel,
    RgbaColor,
    SlintBrush,
    Brush,
    SlintRgbaColor,
    SlintSize,
    SlintPoint,
    SlintImageData
} from "./rust-module";

/**
 *  Represents a two-dimensional point.
 */
export interface Point {
    x: number;
    y: number;
}

/**
 *  Represents a two-dimensional size.
 */
export interface Size {
    width: number;
    height: number;
}

/**
 * This type represents a window towards the windowing system, that's used to render the
 * scene of a component. It provides API to control windowing system specific aspects such
 * as the position on the screen.
 */
export interface Window {
    /**
     * Shows the window on the screen. An additional strong reference on the
     * associated component is maintained while the window is visible.
     */
    show(): void;

    /** Hides the window, so that it is not visible anymore. */
    hide(): void;

    /**
     * Returns the visibility state of the window. This function can return false even if you previously called show()
     * on it, for example if the user minimized the window.
     */
    get isVisible(): boolean;

    /** Gets or sets the logical position of the window on the screen. */
    logicalPosition: Point;

    /** Gets or sets the physical position of the window on the screen. */
    physicalPosition: Point;

    /** Gets or sets the logical size of the window on the screen, */
    logicalSize: Size;

    /** Gets or sets the physical size of the window on the screen, */
    physicalSize: Size;
}

/**
 * An image data type that can be displayed by the Image element.
 *
 * This interface is inspired by the web [ImageData](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) interface.
 */
export interface ImageData {
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

    /**
     * Returns the path of the image, if it was loaded from disk. Otherwise
     * the property is undefined.
     */
    readonly path?: string;
}

/**
 * Model<T> is the interface for feeding dynamic data into
 * `.slint` views.
 *
 * A model is organized like a table with rows of data. The
 * fields of the data type T behave like columns.
 *
 * ### Example
 * As an example let's see the implementation of {@link ArrayModel}
 *
 * ```js
 * export class ArrayModel<T> extends Model<T> {
 *    private a: Array<T>
 *
 *   constructor(arr: Array<T>) {
 *        super();
 *        this.a = arr;
 *    }
 *
 *    rowCount() {
 *        return this.a.length;
 *    }
 *
 *    rowData(row: number) {
 *       return this.a[row];
 *    }
 *
 *    setRowData(row: number, data: T) {
 *        this.a[row] = data;
 *        this.notifyRowDataChanged(row);
 *    }
 *
 *    push(...values: T[]) {
 *        let size = this.a.length;
 *        Array.prototype.push.apply(this.a, values);
 *        this.notifyRowAdded(size, arguments.length);
 *    }
 *
 *    remove(index: number, size: number) {
 *        let r = this.a.splice(index, size);
 *        this.notifyRowRemoved(index, size);
 *    }
 *
 *    get length(): number {
 *        return this.a.length;
 *    }
 *
 *    values(): IterableIterator<T> {
 *        return this.a.values();
 *    }
 *
 *    entries(): IterableIterator<[number, T]> {
 *        return this.a.entries()
 *    }
 *}
 * ```
 */
export abstract class Model<T> {
    #notify: NullPeer;

    constructor() {
        this.#notify = new NullPeer();
    }

    /**
     * Implementations of this function must return the current number of rows.
     */
    abstract rowCount(): number;
    /**
     * Implementations of this function must return the data at the specified row.
     * @param row
     */
    abstract rowData(row: number): T | undefined;
    /**
     * Implementations of this function must store the provided data parameter
     * in the model at the specified row.
     * @param row
     * @param data
     */
    abstract setRowData(row: number, data: T): void;

    protected notifyRowDataChanged(row: number): void {
        this.#notify.rowDataChanged(row);
    }

    protected notifyRowAdded(row: number, count: number): void {
        this.#notify.rowAdded(row, count);
    }

    protected notifyRowRemoved(row: number, count: number): void {
        this.#notify.rowRemoved(row, count);
    }

    protected notifyReset(): void {
        this.#notify.reset();
    }
}

/**
 * @hidden
 */
class NullPeer {
    rowDataChanged(row: number): void { }
    rowAdded(row: number, count: number): void { }
    rowRemoved(row: number, count: number): void { }
    reset(): void { }
}

/**
 * ArrayModel wraps a JavaScript array for use in `.slint` views. The underlying
 * array can be modified with the [[ArrayModel.push]] and [[ArrayModel.remove]] methods.
 */
export class ArrayModel<T> extends Model<T> {
    /**
     * @hidden
     */
    private a: Array<T>;

    /**
     * Creates a new ArrayModel.
     *
     * @param arr
     */
    constructor(arr: Array<T>) {
        super();
        this.a = arr;
    }

    get length(): number {
        return this.a.length;
    }

    rowCount() {
        return this.a.length;
    }

    rowData(row: number) {
        return this.a[row];
    }

    setRowData(row: number, data: T) {
        this.a[row] = data;
        this.notifyRowDataChanged(row);
    }

    /**
     * Pushes new values to the array that's backing the model and notifies
     * the run-time about the added rows.
     * @param values
     */
    push(...values: T[]) {
        let size = this.a.length;
        Array.prototype.push.apply(this.a, values);
        this.notifyRowAdded(size, arguments.length);
    }

    // FIXME: should this be named splice and have the splice api?
    /**
     * Removes the specified number of element from the array that's backing
     * the model, starting at the specified index.
     * @param index
     * @param size
     */
    remove(index: number, size: number) {
        let r = this.a.splice(index, size);
        this.notifyRowRemoved(index, size);
    }

    values(): IterableIterator<T> {
        return this.a.values();
    }

    entries(): IterableIterator<[number, T]> {
        return this.a.entries();
    }
}

/**
 * This interface describes the public API of a Slint component that is common to all instances. Use this to
 * show() the window on the screen, access the window and subsequent window properties, or start the
 * Slint event loop with run().
 */
export interface ComponentHandle {
    /**
     * Shows the window and runs the event loop. The returned promise is resolved when the event loop
     * is terminated, for example when the last window was closed, or {@link quit_event_loop} was called.
     *
     * This function is a convenience for calling {@link show}, followed by {@link run_event_loop}, and
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
    private instance: napi.ComponentInstance;

    /**
     * @hidden
     */
    constructor(instance: napi.ComponentInstance) {
        this.instance = instance;
    }

    get window(): Window {
        return this.instance.window();
    }

    async run() {
        this.show();
        await run_event_loop();
        this.hide();
    }

    show() {
        this.instance.window().show();
    }

    hide() {
        this.instance.window().hide();
    }

    /**
     * @hidden
     */
    get component_instance(): napi.ComponentInstance {
        return this.instance;
    }
}

/**
 * Represents an errors that can be emitted by the compiler.
 */
export class CompileError extends Error {
    public diagnostics: napi.Diagnostic[];

    /**
     * Creates a new CompileError.
     *
     * @param message
     * @param diagnostics
     */
    constructor(message: string, diagnostics: napi.Diagnostic[]) {
        super(message);
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

/**
 * Loads the given Slint file and returns an objects that contains a functions to construct the exported
 * component of the slint file.
 *
 * The following example loads a "Hello World" style Slint file and changes the Text label to a new greeting:
 * `main.slint`:
 * ```
 * export component Main {
 *     in-out property <string> greeting <=> label.text;
 *     label := Text {
 *         text: "Hello World";
 *     }
 * }
 * ```
 *
 * ```js
 * import * as slint from "slint-ui";
 * let ui = slint.loadFile("main.slint");
 * let main = new ui.Main();
 * main.greeting = "Hello friends";
 * ```
 *
 * @param filePath A path to the file to load. If the path is a relative path, then it is resolved
 *                 against the process' working directory.
 * @param options Use {@link LoadFileOptions} to configure additional Slint compilation aspects,
 *                such as include search paths, library imports, or the widget style.
 * @returns The returned object is sealed and provides a property by the name of the component exported
 *          in the `.slint` file. In the above example the name of the property is `Main`. The property
 *          is a constructor function. Use it with the new operator to instantiate the component.
 *          The instantiated object exposes properties and callbacks, and implements the {@link ComponentHandle} interface.
 *          For more details about the exposed properties, see [Instantiating A Component](../index.html#md:instantiating-a-component).
 * @throws {@link CompileError} if errors occur during compilation.
 */
export function loadFile(filePath: string, options?: LoadFileOptions): Object {
    let compiler = new napi.ComponentCompiler();

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

    let definition = compiler.buildFromPath(filePath);

    let diagnostics = compiler.diagnostics;

    if (diagnostics.length > 0) {
        let warnings = diagnostics.filter(
            (d) => d.level == napi.DiagnosticLevel.Warning
        );

        if (typeof options !== "undefined" && options.quiet !== true) {
            warnings.forEach((w) => console.warn("Warning: " + w));
        }

        let errors = diagnostics.filter(
            (d) => d.level == napi.DiagnosticLevel.Error
        );

        if (errors.length > 0) {
            throw new CompileError("Could not compile " + filePath, errors);
        }
    }

    let slint_module = Object.create({});

    Object.defineProperty(slint_module, definition!.name.replace(/-/g, "_"), {
        value: function (properties: any) {
            let instance = definition!.create();

            if (instance == null) {
                throw Error(
                    "Could not create a component handle for" + filePath
                );
            }

            for (var key in properties) {
                let value = properties[key];

                if (value instanceof Function) {
                    instance.setCallback(key, value);
                } else {
                    instance.setProperty(key, properties[key]);
                }
            }

            let componentHandle = new Component(instance!);
            instance!.definition().properties.forEach((prop) => {
                let propName = prop.name.replace(/-/g, "_");

                if (componentHandle[propName] !== undefined) {
                    console.warn("Duplicated property name " + propName);
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
                let callbackName = cb.replace(/-/g, "_");

                if (componentHandle[callbackName] !== undefined) {
                    console.warn("Duplicated callback name " + callbackName);
                } else {
                    Object.defineProperty(componentHandle, cb.replace(/-/g, "_"), {
                        get() {
                            return function () {
                                return instance!.invoke(cb, Array.from(arguments));
                            };
                        },
                        set(callback) {
                            instance!.setCallback(cb, callback);
                        },
                        enumerable: true,
                    });
                }
            });

            // globals
            instance!.definition().globals.forEach((globalName) => {
                if (componentHandle[globalName] !== undefined) {
                    console.warn("Duplicated property name " + globalName);
                } else {
                    let globalObject = Object.create({});

                    instance!.definition().globalProperties(globalName).forEach((prop) => {
                        let propName = prop.name.replace(/-/g, "_");

                        if (globalObject[propName] !== undefined) {
                            console.warn("Duplicated property name " + propName + " on global " + global);
                        } else {
                            Object.defineProperty(globalObject, propName, {
                                get() {
                                    return instance!.getGlobalProperty(globalName, prop.name);
                                },
                                set(value) {
                                    instance!.setGlobalProperty(globalName, prop.name, value);
                                },
                                enumerable: true,
                            });
                        }
                    });

                    instance!.definition().globalCallbacks(globalName).forEach((cb) => {
                        let callbackName = cb.replace(/-/g, "_");

                        if (globalObject[callbackName] !== undefined) {
                            console.warn("Duplicated property name " + cb + " on global " + global);
                        } else {
                            Object.defineProperty(globalObject, cb.replace(/-/g, "_"), {
                                get() {
                                    return function () {
                                        return instance!.invokeGlobal(globalName, cb, Array.from(arguments));
                                    };
                                },
                                set(callback) {
                                    instance!.setGlobalCallback(globalName, cb, callback);
                                },
                                enumerable: true,
                            });
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
    });

    return Object.seal(slint_module);
}

class EventLoop {
    #quit_loop: boolean = false;
    #termination_promise: Promise<unknown> | null = null;
    #terminate_resolve_fn: ((_value: unknown) => void) | null;
    constructor() {
    }

    start(running_callback?: Function): Promise<unknown> {
        if (this.#termination_promise != null) {
            return this.#termination_promise;
        }

        this.#termination_promise = new Promise((resolve) => {
            this.#terminate_resolve_fn = resolve;
        });
        this.#quit_loop = false;

        if (running_callback != undefined) {
            napi.invokeFromEventLoop(() => {
                running_callback();
                running_callback = undefined;
            });
        }

        // Give the nodejs event loop 16 ms to tick. This polling is sub-optimal, but it's the best we
        // can do right now.
        const nodejsPollInterval = 16;
        let id = setInterval(() => {
            if (napi.processEvents() == napi.ProcessEventsResult.Exited || this.#quit_loop) {
                clearInterval(id);
                this.#terminate_resolve_fn!(undefined);
                this.#terminate_resolve_fn = null;
                this.#termination_promise = null;
                return;
            }
        }, nodejsPollInterval);

        return this.#termination_promise;
    }

    quit() {
        this.#quit_loop = true;
    }
}

var global_event_loop: EventLoop = new EventLoop;

/**
 * Spins the Slint event loop and returns a promise that resolves when the loop terminates.
 *
 * If the event loop is already running, then this function returns the same promise as from
 * the earlier invocation.
 *
 * @param running_callback Optional callback that's invoked once when the event loop is running.
 *                         The function's return value is ignored.
 *
 * Note that the event loop integration with Node.js is slightly imperfect. Due to conflicting
 * implementation details between Slint's and Node.js' event loop, the two loops are merged
 * by spinning one after the other, at 16 millisecond intervals. This means that when the
 * application is idle, it continues to consume a low amount of CPU cycles, checking if either
 * event loop has any pending events.
 */
export function run_event_loop(running_callback?: Function): Promise<unknown> {
    return global_event_loop.start(running_callback)
}

/**
 * Stops a spinning event loop. This function returns immediately, and the promise returned
 from run_event_loop() will resolve in a later tick of the nodejs event loop.
 */
export function quit_event_loop() {
    global_event_loop.quit()
}

/**
 * @hidden
 */
export namespace private_api {
    export import mock_elapsed_time = napi.mockElapsedTime;
    export import ComponentCompiler = napi.ComponentCompiler;
    export import ComponentDefinition = napi.ComponentDefinition;
    export import ComponentInstance = napi.ComponentInstance;
    export import ValueType = napi.ValueType;
    export import Window = napi.Window;

    export function send_mouse_click(
        component: Component,
        x: number,
        y: number
    ) {
        component.component_instance.sendMouseClick(x, y);
    }

    export function send_keyboard_string_sequence(
        component: Component,
        s: string
    ) {
        component.component_instance.sendKeyboardStringSequence(s);
    }
}
