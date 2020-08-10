
// Load the native library with `process.dlopen` instead of with `require`.
// This is only done for autotest that do not require nom or neon_cli to
// copy the lib to its right place
function load_native_lib() {
    const os = require('os');
    process.dlopen(module, process.env.SIXTYFPS_NODE_NATIVE_LIB,
        os.constants.dlopen.RTLD_NOW);
    return module.exports;
}

const native = !process.env.SIXTYFPS_NODE_NATIVE_LIB ? require('../native/index.node') : load_native_lib();

require.extensions['.60'] =
    function (module, filename) {
        var c = native.load(filename);
        module.exports[c.name()] = function (init_properties) {
            let comp = c.create(init_properties);
            let ret = {
                show() { comp.show() },
                send_mouse_click(x, y) { comp.send_mouse_click(x, y) }
            };
            c.properties().forEach(x => {
                Object.defineProperty(ret, x, {
                    get() { return comp.get_property(x); },
                    set(newValue) { comp.set_property(x, newValue); },
                    enumerable: true,
                })
            });
            c.signals().forEach(x => {
                Object.defineProperty(ret, x, {
                    get() { return function () { comp.emit_signal(x); } },
                    enumerable: true,
                })
            });
            return ret;
        }
    }

module.exports = native;
