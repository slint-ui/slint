// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import * as path from "path";

import * as napi from "./rust-module";
export { Diagnostic, DiagnosticLevel, Window, Brush, Color, ImageData, Point, Size, SlintModelNotify } from "./rust-module";

/**
 * ModelPeer is the interface that the run-time implements. An instance is
 * set on dynamic {@link Model} instances and can be used to notify the run-time
 * of changes in the structure or data of the model.
 */
export interface ModelPeer {
    /**
     * Call this function from our own model to notify that fields of data
     * in the specified row have changed.
     * @argument row
     */
    rowDataChanged(row: number): void;
    /**
     * Call this function from your own model to notify that one or multiple
     * rows were added to the model, starting at the specified row.
     * @param row
     * @param count
     */
    rowAdded(row: number, count: number): void;
    /**
     * Call this function from your own model to notify that one or multiple
     * rows were removed from the model, starting at the specified row.
     * @param row
     * @param count
     */
    rowRemoved(row: number, count: number): void;

    /**
     * Call this function from your own model to notify that the model has been
     * changed and everything must be reloaded
     */
    reset(): void;
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
 * export class ArrayModel<T> implements Model<T> {
 *    private a: Array<T>
 *    notify: ModelPeer;
 *
 *   constructor(arr: Array<T>) {
 *        this.a = arr;
 *        this.notify = new NullPeer();
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
 *        this.notify.rowDataChanged(row);
 *    }
 *
 *    push(...values: T[]) {
 *        let size = this.a.length;
 *        Array.prototype.push.apply(this.a, values);
 *        this.notify.rowAdded(size, arguments.length);
 *    }
 *
 *    remove(index: number, size: number) {
 *        let r = this.a.splice(index, size);
 *        this.notify.rowRemoved(index, size);
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
export interface Model<T> {
    /**
     * Implementations of this function must return the current number of rows.
     */
    rowCount(): number;
    /**
     * Implementations of this function must return the data at the specified row.
     * @param row
     */
    rowData(row: number): T | undefined;
    /**
     * Implementations of this function must store the provided data parameter
     * in the model at the specified row.
     * @param row
     * @param data
     */
    setRowData(row: number, data: T): void;
    /**
     * This public member is set by the run-time and implementation must use this
     * to notify the run-time of changes in the model.
     */
    notify: ModelPeer;
}

/**
 * @hidden
 */
class NullPeer implements ModelPeer {
    rowDataChanged(row: number): void { }
    rowAdded(row: number, count: number): void { }
    rowRemoved(row: number, count: number): void { }
    reset(): void { }
}

/**
 * ArrayModel wraps a JavaScript array for use in `.slint` views. The underlying
 * array can be modified with the [[ArrayModel.push]] and [[ArrayModel.remove]] methods.
 */
export class ArrayModel<T> implements Model<T> {
    /**
     * @hidden
     */
    private a: Array<T>
    notify: ModelPeer;

    /**
     * Creates a new ArrayModel.
     *
     * @param arr
     */
    constructor(arr: Array<T>) {
        this.a = arr;
        this.notify = new NullPeer();
    }

    rowCount() {
        return this.a.length;
    }
    rowData(row: number) {
        return this.a[row];
    }
    setRowData(row: number, data: T) {
        this.a[row] = data;
        this.notify.rowDataChanged(row);
    }
    /**
     * Pushes new values to the array that's backing the model and notifies
     * the run-time about the added rows.
     * @param values
     */
    push(...values: T[]) {
        let size = this.a.length;
        Array.prototype.push.apply(this.a, values);
        this.notify.rowAdded(size, arguments.length);
    }
    // FIXME: should this be named splice and have the splice api?
    /**
     * Removes the specified number of element from the array that's backing
     * the model, starting at the specified index. This is equivalent to calling
     * Array.slice() on the array and notifying the run-time about the removed
     * rows.
     * @param index
     * @param size
     */
    remove(index: number, size: number) {
        let r = this.a.splice(index, size);
        this.notify.rowRemoved(index, size);
    }

    get length(): number {
        return this.a.length;
    }

    values(): IterableIterator<T> {
        return this.a.values();
    }

    entries(): IterableIterator<[number, T]> {
        return this.a.entries()
    }
}

/**
 * This interface describes the public API of a Slint component that is common to all instances. Use this to
 * show() the window on the screen, access the window and subsequent window properties, or start the
 * Slint event loop with run().
 */
export interface ComponentHandle {
    /**
     * Shows the window and runs the event loop.
     */
    run();

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
    get window(): napi.Window;
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

    run() {
        this.instance.run();
    }

    show() {
        this.instance.window().show();
    }

    hide() {
        this.instance.window().hide();
    }

    get window(): napi.Window {
        return this.instance.window();
    }

    /**
    * @hidden
    */
    get component_instance(): napi.ComponentInstance {
        return this.instance;
    }
}

/**
 * @hidden
 */
interface Callback {
    (): any;
    setHandler(cb: any): void;
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
 * Loads the given slint file and returns a constructor to create an instance of the exported component.
 */
export function loadFile(filePath: string) : Object {
    // this is a workaround that fixes an issue there resources in slint files cannot be loaded if the
    // file path is given as relative path
    let absoluteFilePath = path.resolve(filePath);
    let compiler = new napi.ComponentCompiler;
    let definition = compiler.buildFromPath(absoluteFilePath);

    let diagnostics = compiler.diagnostics;

    if (diagnostics.length > 0) {
        let warnings = diagnostics.filter((d) => d.level == napi.DiagnosticLevel.Warning);
        warnings.forEach((w) => console.log("Warning: " + w));

        let errors = diagnostics.filter((d) => d.level == napi.DiagnosticLevel.Error);

        if (errors.length > 0) {
            throw new CompileError("Could not compile " + path, errors);
        }
    }

    let slint_module = Object.create({});

    Object.defineProperty(slint_module, definition!.name.replace(/-/g, '_'), {
        value: function(properties: any) {
            let instance = definition!.create();

            if (instance == null) {
                throw Error("Could not create a component handle for" + path);
            }

            for(var key in properties) {
                let value = properties[key];

                if (value instanceof Function) {
                    instance.setCallback(key, value);
                } else {
                    instance.setProperty(key, properties[key]);
                }
            }

            let componentHandle = new Component(instance!);
            instance!.definition().properties.forEach((prop) => {
                Object.defineProperty(componentHandle, prop.name.replace(/-/g, '_') , {
                    get() { return instance!.getProperty(prop.name); },
                    set(value) { instance!.setProperty(prop.name, value); },
                    enumerable: true
                })
            });

            instance!.definition().callbacks.forEach((cb) => {
                Object.defineProperty(componentHandle, cb.replace(/-/g, '_') , {
                    get() {
                        let callback = function () { return instance!.invoke(cb, Array.from(arguments)); } as Callback;
                        callback.setHandler = function (callback) { instance!.setCallback(cb, callback) };
                        return callback;
                    },
                    enumerable: true,
                })
            });

            return componentHandle;
        },
    });

    return slint_module;
}

// This api will be removed after teh event loop handling is merged check PR #3718.
// After that this in no longer necessary.
export namespace Timer {
    export function singleShot(duration: number, handler: () => void) {
        napi.singleshotTimer(duration, handler)
    }
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

    export function send_mouse_click(component: Component, x: number, y: number) {
        component.component_instance.sendMouseClick(x, y);
    }

    export function send_keyboard_string_sequence(component: Component, s: string) {
        component.component_instance.sendKeyboardStringSequence(s);
    }
}