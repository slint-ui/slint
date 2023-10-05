// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import { ComponentCompiler, ComponentInstance, Window, DiagnosticLevel } from "./napi";
export * from "./napi";

/**
 * ModelPeer is the interface that the run-time implements. An instance is
 * set on dynamic Model<T> instances and can be used to notify the run-time
 * of changes in the structure or data of the model.
 */
interface ModelPeer {
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
 */
interface Model<T> {
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

export interface ComponentHandle {
    run();
    show();
    hide();
    get window(): Window;
}

class Component implements ComponentHandle {
    private instance: ComponentInstance;

    /**
    * @hidden
    */
    constructor(instance: ComponentInstance) {
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

    get window(): Window {
        return this.instance.window();
    }
}

/**
 * @hidden
 */
interface Callback {
    (): any;
    setHandler(cb: any): void;
}

export function loadFile(path: string) : Object {
    let compiler = new ComponentCompiler;
    let definition = compiler.buildFromPath(path);


    let diagnostics = compiler.diagnostics;

    if (diagnostics.length > 0) {
        diagnostics.forEach((d) => {
            if (d.level == DiagnosticLevel.Warning) {
                console.log(d.message);
            } else {
                throw Error(d.message);
            }
        })
    }

    let slint_module = Object.create({});

    Object.defineProperty(slint_module, definition!.name, {
        value: function() {
            let instance = definition!.create();

            if (instance == null) {
                throw Error("Could not create a component handle for" + path);
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
                            let callback = function () { return instance!.invoke(cb, [...arguments]); } as Callback;
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

