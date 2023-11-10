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

## Prerequisites

To use Slint with Node.js, ensure the following programs are installed:

  * **[Node.js](https://nodejs.org/download/release/)** (v16. or newer)
  * **[npm](https://www.npmjs.com/)**
  * **[Rust compiler](https://www.rust-lang.org/tools/install)** (1.70 or newer)

Depending on your operating system, you may need additional components. For a list of required system libraries,
see <https://github.com/slint-ui/slint/blob/master/docs/building.md#prerequisites>.

## Getting Started

1. In a new directory, create a new Node.js project by calling [`npm init`](https://docs.npmjs.com/cli/v10/commands/npm-init).
2. Install Slint for your project using [`npm install slint-ui`](https://docs.npmjs.com/cli/v10/commands/npm-install).
3. Create a new file called `main.slint` with the following contents:

```
import { AboutSlint, Button, VerticalBox } from "std-widgets.slint";
export component Demo {
    in-out property <string> greeting <=> label.text;
    VerticalBox {
        alignment: start;
        label := Text {
            text: "Hello World!";
            font-size: 24px;
            horizontal-alignment: center;
        }
        AboutSlint {
            preferred-height: 150px;
        }
        HorizontalLayout { alignment: center; Button { text: "OK!"; } }
    }
}
```

This file declares the user interface.

4. Create a new file called `index.mjs` with the following contents:

```
import * as slint from "slint-ui";
let ui = slint.loadFile("main.slint");
let demo = new ui.Demo();

await demo.run();
```

This is your main JavaScript entry point:

* Import the Slint API as an [ECMAScript module](https://nodejs.org/api/esm.html#modules-ecmascript-modules) module. If you prefer you can
  also import it as [CommonJS](https://nodejs.org/api/modules.html#modules-commonjs-modules) module.
* Invoke `loadFile()` to compile and load the `.slint` file.
* Instantiate the `Demo` component declared in `main.slint`.
* Run it by showing it on the screen and reacting to user input.

5. Run the example with `node index.mjs`

For a complete example, see [/examples/todo/node](https://github.com/slint-ui/slint/tree/master/examples/todo/node).

## API Overview

### Instantiating a Component

Use the {@link loadFile} function to load a `.slint` file. Instantiate the [exported component](../slint/src/language/concepts/file)
with the new operator. Access exported callbacks and properties as JavaScript properties on the instantiated component. In addition,
the returned object implements the {@link ComponentHandle} interface, to show/hide the instance or access the window.

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

### Accessing a Properties

[Properties](../slint/src/language/syntax/properties) declared as `out` or `in-out` in `.slint` files are visible as JavaScript properties on the component instance.

**`main.slint`**
export component MainWindow {
    in-out property <string> name;
    in-out property <int> age: 42;
}

```js
let ui = slint.loadFile("main.slint");
let instance = new ui.MainWindow();
console.log(instance.age); // Prints 42
instance.name = "Joe";
```

### Setting and Invoking Callbacks

[Callbacks](src/language/syntax/callbacks) declared in `.slint` files are visible as JavaScript function properties on the component instance. Invoke them
as function to invoke the callback, and assign JavaScript functions to set the callback handler.

**`ui/my-component.slint`**

```
export component MyComponent inherits Window {
    callback clicked <=> i-touch-area.clicked;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

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
| `bool` | `Boolean` | |
| `float` | `Number` | |
| `string` | `String` | |
| `color` | {@link RgbaColor} | |
| `brush` | {@link Brush} | |
| `image` | {@link ImageData} | |
| `length` | `Number` | |
| `physical_length` | `Number` | |
| `duration` | `Number` | The number of milliseconds |
| `angle` | `Number` | The angle in degrees |
| `relative-font-size` | `Number` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
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
