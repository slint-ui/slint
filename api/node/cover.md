<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Slint-node (Beta)

[![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/slint-ui)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-node is the integration with Node.js.

To get started you use the [walk-through tutorial](https://slint.dev/docs/tutorial/node).
We also have a [Getting Started Template](https://github.com/slint-ui/slint-nodejs-template) repository with
the code of a minimal application using Slint that can be used as a starting point to your program.

**Warning: Beta**
Slint-node is still in the early stages of development: APIs will change and important features are still being developed.

## Slint Language Manual

The [Slint Language Documentation](../slint) covers the Slint UI description language
in detail.

## Installing Slint

Slint is available via NPM, so you can install by running the following command:

```sh
npm install slint-ui
```

### Dependencies

You need to install the following components:

  * **[Node.js](https://nodejs.org/download/release/)** (v16. or newer)
  * **[npm](https://www.npmjs.com/)**
  * **[Rust compiler](https://www.rust-lang.org/tools/install)** (1.70 or newer)

You will also need a few more dependencies, see <https://github.com/slint-ui/slint/blob/master/docs/building.md#prerequisites>

## Using Slint

First, import the API from the `slint-ui` module. In the following examples we're using [ECMAScript module syntax](https://nodejs.org/api/esm.html#modules-ecmascript-modules), but if you prefer you can also import the API using [CommonJS](https://nodejs.org/api/modules.html#modules-commonjs-modules) syntax.

To initialize the API, you first need to import the `slint-ui` module in our code:

```js
import * as slint from "slint-ui";
```

Next, load a slint file with the `loadFile` function:

```js
let ui = slint.loadFile("ui/main.slint");
```

Combining these two steps leads us to the obligatory "Hello World" example:

```js
import * as slint from "slint-ui";
let ui = slint.loadFile(".ui/main.slint");
let main = new ui.Main();
main.run();
```

For a full example, see [/examples/todo/node](https://github.com/slint-ui/slint/tree/master/examples/todo/node).

## API Overview

### Instantiating a Component

The following example shows how to instantiating a Slint component from JavaScript.

**`ui/main.slint`**

```
export component MainWindow inherits Window {
    callback clicked <=> i-touch-area.clicked;

    in property <int> counter;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The exported component is exposed as a type constructor. The type constructor takes as parameter
an object which allow to initialize the value of public properties or callbacks.

**`main.js`**

```js
import * as slint from "slint-ui";
// In this example, the main.slint file exports a module which
// has a counter property and a clicked callback
let ui = slint.loadFile("ui/main.slint");
let component = new ui.MainWindow({
    counter: 42,
    clicked: function() { console.log("hello"); }
});
```

### Accessing a property

Properties declared as `out` or `in-out` in `.slint` files are visible as JavaScript on the component instance.

```js
component.counter = 42;
console.log(component.counter);
```

### Callbacks

Callback in Slint can be defined usign the `callback` keyword and can be connected to a callback of an other component
usign the `<=>` syntax.

**`ui/my-component.slint`**

```
export component MyComponent inherits Window {
    callback clicked <=> i-touch-area.clicked;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The callbacks in JavaScript are exposed as property and that can be called as a function.

**`main.js`**

```js
import * as slint from "slint-ui";

let ui = slint.loadFile("ui/my-component.slint");
let component = new ui.MyComponent();

// connect to a callback
component.clicked = function() { console.log("hello"); };
// emit a callback
component.clicked();
```

### Type Mappings

The types used for properties in .slint design markup each translate to specific types in JavaScript. The follow table summarizes the entire mapping:

| `.slint` Type | JavaScript Type | Notes |
| --- | --- | --- |
| `int` | `Number` | |
| `float` | `Number` | |
| `string` | `String` | |
| `color` | {@link RgbaColor} |  |
| `brush` | {@link Brush} |  |
| `image` | {@link ImageData} |  |
| `length` | `Number` | |
| `physical_length` | `Number` | |
| `duration` | `Number` | The number of milliseconds |
| `angle` | `Number` | The angle in degrees |
| structure | `Object` | Structures are mapped to JavaScript objects where each structure field is a property. |
| array | `Array` or any implementation of {@link Model} | |

### Arrays and Models

[Array properties](../slint/src/language/syntax/types#arrays-and-models) can be set from JavaScript by passing
either `Array` objects or implementations of the {@link Model} interface.

When passing a JavaScript `Array` object, the contents of the array are copied. Any changes to the JavaScript afterwards will not be visible on the Slint side. Similarly, reading a Slint array property from JavaScript that was
previously initialised from a JavaScript `Array`, will return a newly allocated JavaScript `Array`.

```js
component.model = [1, 2, 3];
// component.model.push(4); // does not work, because assignment creates a copy.
// Use re-assignment instead.
component.model = component.model.concat(4);
```

Another option is to set an object that implements the {@link Model} interface. Rreading a Slint array property from JavaScript that was previously initialised from a {@link Model} object, will return a reference to the model.
