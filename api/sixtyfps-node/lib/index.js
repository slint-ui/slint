
native = require('../native/index.node');

require.extensions['.60'] = function (module, filename) {
    var c = native.load(filename);
    module.exports[c.name()] = function (init_properties) {
        return c.create(init_properties);
    }
}

module.exports = native;



