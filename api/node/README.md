<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint-node (Beta)

[![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/slint-ui)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-node is the integration with Node.js.

To get started you use the [walk-through tutorial](https://slint.dev/docs/slint/tutorial/quickstart).
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
  * **[pnpm](https://www.pnpm.io/)**
  * **[Rust compiler](https://www.rust-lang.org/tools/install)**

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
let ui = slint.loadFile(new URL(".ui/main.slint", import.meta.url));
let main = new ui.Main();
main.run();
```

For a full example, see [/examples/todo/node](https://github.com/slint-ui/slint/tree/master/examples/todo/node).

## API Overview

### Instantiating a Component

The following example shows how to instantiate a Slint component from JavaScript.

**`ui/main.slint`**

```slint
export component MainWindow inherits Window {
    callback clicked <=> i-touch-area.clicked;

    in property <int> counter;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

Each exported Window component is exposed as a type constructor. The type constructor takes as parameter
an object which allow to initialize the value of public properties or callbacks.

**`main.js`**

```js
import * as slint from "slint-ui";
// In this example, the main.slint file exports a module which
// has a counter property and a clicked callback
let ui = slint.loadFile(new URL("ui/main.slint", import.meta.url));
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

Callbacks in Slint can be defined using the `callback` keyword and can be connected to a callback of an other component
using the `<=>` syntax.

**`ui/my-component.slint`**

```slint
export component MyComponent inherits Window {
    callback clicked <=> i-touch-area.clicked;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The callbacks in Slint are exposed as properties in JavaScript and that can be called as a function.

**`main.js`**

```js
import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("ui/my-component.slint", import.meta.url));
let component = new ui.MyComponent();

// connect to a callback
component.clicked = function() { console.log("hello"); };
// emit a callback
component.clicked();
```

### Functions

Functions in Slint can be defined using the `function` keyword.

**`ui/my-component.slint`**

```slint
export component MyComponent inherits Window {
    width: 400px;
    height: 200px;

    public function my-function() -> int {
        return 42;
    }
}
```

If the function is marked `public`, it can also be called from JavaScript.

**`main.js`**

```js
import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("ui/my-component.slint", import.meta.url));
let component = new ui.MyComponent();

// call a public function
let result = component.my_function();
```

### Type Mappings

The types used for properties in .slint design markup each translate to specific types in JavaScript. The follow table summarizes the entire mapping:

| `.slint` Type | JavaScript Type | Notes |
| --- | --- | --- |
| `int` | `Number` | |
| `float` | `Number` | |
| `string` | `String` | |
| `color` | `RgbaColor` |  |
| `brush` | `Brush` |  |
| `image` | `ImageData` |  |
| `length` | `Number` | |
| `physical_length` | `Number` | |
| `duration` | `Number` | The number of milliseconds |
| `angle` | `Number` | The angle in degrees |
| structure | `Object` | Structures are mapped to JavaScript objects where each structure field is a property. |
| array | `Array` or any implementation of Model | |
| enumeration | `String` | The value of an enum |

### Arrays and Models

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

As an example, here is the implementation of the `ArrayModel` (which is available as `slint.ArrayModel`)

```js
import * as slint from "slint-ui";

let array = [1, 2, 3];

export class ArrayModel<T> extends slint.Model<T> {
    private a: Array<T>

   constructor(arr: Array<T>) {
        super();
        this.a = arr;
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

    push(...values: T[]) {
        let size = this.a.length;
        Array.prototype.push.apply(this.a, values);
        this.notify.rowAdded(size, arguments.length);
    }

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

let model = new ArrayModel(array);

component.model = model;
model.push(4); // this works
// does NOT work, getting the model does not return the right object
// component.model.push(5);
```

### structs

An exported struct can be created either by defing of an object literal or by using the new keyword.

**`my-component.slint`**

```slint
export struct Person {
    name: string,
    age: int
}

export component MyComponent inherits Window {
    in-out property <Person> person;
}
```

**`main.js`**

```js

import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("my-component.slint", import.meta.url));
let component = new ui.MyComponent();

// object literal
component.person = { name: "Peter", age: 22 };

// new keyword (sets property values to default e.g. '' for string)
component.person = new ui.Person();

// new keyword with parameters
component.person = new ui.Person({ name: "Tim", age: 30 });
```

### enums

A value of an exported enum can be set as string or by using the value from the exported enum.

**`my-component.slint`**

```slint
export enum Position {
    top,
    bottom
}

export component MyComponent inherits Window {
    in-out property <Position> position;
}
```

**`main.js`**

```js

import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("my-component.slint", import.meta.url));
let component = new ui.MyComponent();

// set enum value as string
component.position = "top";

// use the value of the enum
component.position = ui.Position.bottom;
```
