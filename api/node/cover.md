<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Slint-node (Beta)

[![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/slint-ui)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-node is the integration with node.

To get started you can use the [Walk-through tutorial](https://slint.dev/docs/tutorial/node).
We also have a [Getting Started Template](https://github.com/slint-ui/slint-nodejs-template) repository with
the code of a minimal application using Slint that can be used as a starting point to your program.

**Warning: Beta**
Slint-node is still in the early stages of development: APIs will change and important features are still being developed.

## Slint Language Manual

The [Slint language manual](../slint) covers the Slint UI description language
in detail.

## Installing Slint

Slint is available via NPM, so you can install by running the following command:

```sh
npm install slint-ui
```

### Dependencies

You need to install the following components:

  * **[Node.js](https://nodejs.org/download/release/v16.19.1/)** (v16. Newer versions currently not supported: [#961](https://github.com/slint-ui/slint/issues/961))
  * **[npm](https://www.npmjs.com/)**
  * **[Rust compiler](https://www.rust-lang.org/tools/install)** (1.66 or newer)

You will also need a few more dependencies, see <https://github.com/slint-ui/slint/blob/master/docs/building.md#prerequisites>

## Using Slint

To initialize the API, you first need to import the `slint-ui` module in our code:

```js
let slint = require("slint-ui");
```

This step also installs a hook in NodeJS that allows you to import `.slint` files directly:

```js
let ui = require("../ui/main.slint");
```

Combining these two steps leads us to the obligatory "Hello World" example:

```js
require("slint-ui");
let ui = require("../ui/main.slint");
let main = new ui.Main();
main.run();
```

See [/examples/todo/node](https://github.com/slint-ui/slint/tree/master/examples/todo/node) for a full example.

## API Overview

### Instantiating a component

The exported component is exposed as a type constructor. The type constructor takes as parameter
an object which allow to initialize the value of public properties or callbacks.

```js
require("slint-ui");
// In this example, the main.slint file exports a module which
// has a counter property and a clicked callback
let ui = require("ui/main.slint");
let component = new ui.MainWindow({
    counter: 42,
    clicked: function() { console.log("hello"); }
});
```

### Accessing a property

Properties are exposed as properties on the component instance

```js
component.counter = 42;
console.log(component.counter);
```

### Callbacks

The callbacks are also exposed as property that have a setHandler function, and that can can be called.

```js
// connect to a callback
component.clicked.setHandler(function() { console.log("hello"); })
// emit a callback
component.clicked();
```

### Type Mappings

| `.slint` Type | JavaScript Type | Notes |
| --- | --- | --- |
| `int` | `Number` | |
| `float` | `Number` | |
| `string` | `String` | |
| `color` | `String` | Colors are represented as strings in the form `"#rrggbbaa"`. When setting a color property, any CSS compliant color is accepted as a string. |
| `length` | `Number` | |
| `physical_length` | `Number` | |
| `duration` | `Number` | The number of milliseconds |
| `angle` | `Number` | The value in degrees |
| structure | `Object` | Structures are mapped to JavaScrip objects with structure fields mapped to properties. |
| array | `Array` or Model Object | |

### Models

For property of array type, they can either be set using an array.
In that case, getting the property also return an array.
If the array was set within the .slint file, the array can be obtained

```js
component.model = [1, 2, 3];
// component.model.push(4); // does not work, because it operate on a copy
// but re-assigning works
component.model = component.model.concat(4);
```

Another option is to set a model object.  A model object has the following function:

* `rowCount()`: returns the number of element in the model.
* `rowData(index)`: return the row at the given index
* `setRowData(index, data)`: called when the model need to be changed. `this.notify.rowDataChanged` must be called if successful.

When such an object is set to a model property, it gets a new `notify` object with the following function

* `rowDataChanged(index)`: notify the view that the row was changed.
* `rowAdded(index, count)`: notify the view that rows were added.
* `rowRemoved(index, count)`: notify the view that a row were removed.
* `reset()`: notify the view that everything may have changed.

As an example, here is the implementation of the `ArrayModel` (which is available as `slint.ArrayModel`)

```js
let array = [1, 2, 3];
let model = {
    rowCount() { return a.length; },
    rowData(row) { return a[row]; },
    setRowData(row, data) { a[row] = data; this.notify.rowDataChanged(row); },
    push() {
        let size = a.length;
        Array.prototype.push.apply(a, arguments);
        this.notify.rowAdded(size, arguments.length);
    },
    remove(index, size) {
        let r = a.splice(index, size);
        this.notify.rowRemoved(size, arguments.length);
    },
};
component.model = model;
model.push(4); // this works
// does NOT work, getting the model does not return the right object
// component.model.push(5);
```
