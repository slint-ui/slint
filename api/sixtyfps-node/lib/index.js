
native = require('../native/index.node');

require.extensions['.60'] = function (module, filename) {
    var c = native.load(filename);
    module.exports.show = function () {
        c.show();
    }

}

module.exports = native;



