
native = require('../native/index.node');

require.extensions['.60'] =
    function (module, filename) {
        var c = native.load(filename);
        module.exports[c.name()] = function (init_properties) {
            let comp = c.create(init_properties);
            let ret = { show() { comp.show() } };
            c.properties().forEach(x => {
                Object.defineProperty(ret, x, {
                    get() { return comp.get_property(x); },
                    set(newValue) { comp.set_property(x, newValue); },
                    enumerable: true,
                })
            });
            return ret;
        }
    }

module.exports = native;
