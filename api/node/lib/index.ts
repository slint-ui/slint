// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// Load the native library with `process.dlopen` instead of with `require`.
// This is only done for autotest that do not require nom or neon_cli to
// copy the lib to its right place
/**
 * @hidden
 */
function load_native_lib() {
    const os = require('os');
    (process as any).dlopen(module, process.env.SLINT_NODE_NATIVE_LIB,
        os.constants.dlopen.RTLD_NOW);
    return module.exports;
}

/**
 * @hidden
 */
let native = !process.env.SLINT_NODE_NATIVE_LIB ? require('../native/index.node') : load_native_lib();

/**
 * @hidden
 */
class Component {
    protected comp: any;

    constructor(comp: any) {
        this.comp = comp;
    }

    run() {
        this.comp.run();
    }

    show() {
        this.window.show();
    }

    hide() {
        this.window.hide()
    }

    get window(): SlintWindow {
        return new WindowAPI(this.comp.window());
    }

    send_mouse_click(x: number, y: number) {
        this.comp.send_mouse_click(x, y)
    }

    send_keyboard_string_sequence(s: String) {
        this.comp.send_keyboard_string_sequence(s)
    }
}

interface Point {
    x: number;
    y: number;
}

interface Size {
    width: number;
    height: number;
}

interface SlintWindow {
    show(): void;
    hide(): void;
    is_visible: boolean;
    logical_position: Point;
    physical_position: Point;
    logical_size: Size;
    physical_size: Size;
}

/**
 * @hidden
 */
class WindowAPI implements SlintWindow {
    protected impl: any;

    constructor(impl: any) {
        this.impl = impl;
    }

    show(): void {
        this.impl.show();
    }
    hide(): void {
        this.impl.hide();
    }
    get is_visible(): boolean {
        return this.impl.get_is_visible();
    }
    get logical_position(): Point {
        return this.impl.get_logical_position();
    }
    set logical_position(pos: Point) {
        this.impl.set_logical_position(pos);
    }
    get physical_position(): Point {
        return this.impl.get_physical_position();
    }
    set physical_position(pos: Point) {
        this.impl.set_physical_position(pos);
    }
    get logical_size(): Size {
        return this.impl.get_logical_size();
    }
    set logical_size(size: Size) {
        this.impl.set_logical_size(size);
    }
    get physical_size(): Size {
        return this.impl.get_physical_size();
    }
    set physical_size(size: Size) {
        this.impl.set_physical_size(size);
    }
}

/**
 * @hidden
 */
interface Callback {
    (): any;
    setHandler(cb: any): void;
}

require.extensions['.60'] = require.extensions['.slint'] =
    function (module, filename) {
        var c = native.load(filename);
        module.exports[c.name().replace(/-/g, '_')] = function (init_properties: any) {
            let comp = c.create(init_properties);
            let ret = new Component(comp);
            c.properties().forEach((x: string) => {
                Object.defineProperty(ret, x.replace(/-/g, '_'), {
                    get() { return comp.get_property(x); },
                    set(newValue) { comp.set_property(x, newValue); },
                    enumerable: true,
                })
            });
            c.callbacks().forEach((x: string) => {
                Object.defineProperty(ret, x.replace(/-/g, '_'), {
                    get() {
                        let callback = function () { return comp.invoke_callback(x, [...arguments]); } as Callback;
                        callback.setHandler = function (callback) { comp.connect_callback(x, callback) };
                        return callback;
                    },
                    enumerable: true,
                })
            });
            return ret;
        }
    }

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
    rowData(row: number): T;
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
class ArrayModel<T> implements Model<T> {
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

class SlintImageData {
    _data: Uint8ClampedArray
    _width: number;
    _height: number;

    constructor(data: Uint8ClampedArray, width: number, height?: number) {
        this._data = data;

        this._width = width;

        if (height === undefined) {
            this._height = data.length / width / 4;
        } else {
            this._height = height;
        }
    }

    get width(): number {
        return this._width;
    }

    get height(): number {
        return this._height;
    }

    get data(): Array<number> {
        return Array.from(this._data);
    }
}

class SlintImage {
    _path: string | undefined;
    _data: SlintImageData | undefined;

    constructor(path?: string, data?: SlintImageData) {
        if (path !== undefined) {
            this._path = path;
        }

        if (data !== undefined) {
            this._data = data;
        }
    }

    public get value(): string | SlintImageData | undefined {
        if (this._path !== undefined) {
            return this._path;
        }

        if (this._data !== undefined) {
            return this._data;
        }

        return undefined;
    }
}

module.exports = {
    private_api: native,
    ArrayModel: ArrayModel,
    Timer: {
        singleShot: native.singleshot_timer,
    },
    Image: SlintImage,
    ImageData: SlintImageData
};
