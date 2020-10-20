/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

// Load the native library with `process.dlopen` instead of with `require`.
// This is only done for autotest that do not require nom or neon_cli to
// copy the lib to its right place
function load_native_lib() {
    const os = require('os');
    (process as any).dlopen(module, process.env.SIXTYFPS_NODE_NATIVE_LIB,
        os.constants.dlopen.RTLD_NOW);
    return module.exports;
}

let native = !process.env.SIXTYFPS_NODE_NATIVE_LIB ? require('../native/index.node') : load_native_lib();

class Component {
    protected comp: any;

    constructor(comp: any) {
        this.comp = comp;
    }

    show() {
        this.comp.show();
    }

    send_mouse_click(x: number, y: number) {
        this.comp.send_mouse_click(x, y)
    }

    send_keyboard_string_sequence(s: String) {
        this.comp.send_keyboard_string_sequence(s)
    }
}

require.extensions['.60'] =
    function (module, filename) {
        var c = native.load(filename);
        module.exports[c.name()] = function (init_properties: any) {
            let comp = c.create(init_properties);
            let ret = new Component(comp);
            c.properties().forEach((x: string) => {
                Object.defineProperty(ret, x, {
                    get() { return comp.get_property(x); },
                    set(newValue) { comp.set_property(x, newValue); },
                    enumerable: true,
                })
            });
            c.signals().forEach((x: string) => {
                Object.defineProperty(ret, x, {
                    get() { return function () { comp.emit_signal(x, [...arguments]); } },
                    enumerable: true,
                })
            });
            return ret;
        }
    }

interface ModelPeer {
    rowDataChanged(row: number): void;
    rowAdded(row: number, count: number): void;
    rowRemoved(row: number, count: number): void;
}

class NullPeer implements ModelPeer {
    rowDataChanged(row: number): void { }
    rowAdded(row: number, count: number): void { }
    rowRemoved(row: number, count: number): void { }
}

/**
 * ArrayModel wraps a JavaScript array for use in `.60` views.
*/
class ArrayModel<T> {
    private a: Array<T>
    private notify: ModelPeer;

    /**
     * @template T
     * Creates a new ArrayModel.
     * 
     * @param {Array<T>} arr
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
     * Pushes new values to the array that's backing the model.
     * @param {T} values 
     */
    push(...values: T[]) {
        let size = this.a.length;
        Array.prototype.push.apply(this.a, values);
        this.notify.rowAdded(size, arguments.length);
    }
    // FIXME: should this be named splice and hav ethe splice api?
    remove(index: number, size: number) {
        let r = this.a.splice(index, size);
        this.notify.rowRemoved(size, arguments.length);
    }
}

native.ArrayModel = ArrayModel;

module.exports = native;
