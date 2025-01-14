<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Slint-node (Beta)

[![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/slint-ui)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-node is the integration with [Node.js](https://nodejs.org/en) and [Deno](https://deno.com).

To get started you use the [walk-through tutorial](https://slint.dev/docs/slint/tutorial/quickstart).
We also have a [Getting Started Template](https://github.com/slint-ui/slint-nodejs-template) repository with
the code of a minimal application using Slint that can be used as a starting point to your program.

**Warning: Beta**
Slint-node is still in the early stages of development: APIs will change and important features are still being developed.

## Slint Language Manual

The [Slint Language Documentation](http://slint.dev/docs/slint) covers the Slint UI description language
in detail.

## Prerequisites

To use Slint with Node.js, ensure the following programs are installed:

  * **[Node.js](https://nodejs.org/download/release/)** (v16. or newer)
  * **[npm](https://www.npmjs.com/)**

To use Slint with Deno, ensure the following programs are installed:

  * **[Deno](https://docs.deno.com/runtime/manual)**

### Building from Source

Slint-node comes with pre-built binaries for macOS, Linux, and Windows. If you'd like to use Slint-node on a system
without pre-built binaries, you need to additional software:

  * **[Rust compiler](https://www.rust-lang.org/tools/install)** (1.82 or newer) * Depending on your operating system, you may need additional components. For a list of required system libraries,
    see <https://github.com/slint-ui/slint/blob/master/docs/building.md#prerequisites>.

## Getting Started (Node.js)

1. In a new directory, create a new Node.js project by calling [`npm init`](https://docs.npmjs.com/cli/v10/commands/npm-init).
2. Install Slint for your project using [`npm install slint-ui`](https://docs.npmjs.com/cli/v10/commands/npm-install).
3. Create a new file called `main.slint` with the following contents:

```
import { AboutSlint, Button, VerticalBox } from "std-widgets.slint";
export component Demo inherits Window {
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

```js
import * as slint from "slint-ui";
let ui = slint.loadFile(new URL("main.slint", import.meta.url));
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

## Getting Started (Deno)

1. Create a new file called `main.slint` with the following contents:

```
import { AboutSlint, Button, VerticalBox } from "std-widgets.slint";
export component Demo inherits Window {
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

2. Create a new file called `deno.json` (a [Deno Import Map](https://docs.deno.com/runtime/manual/basics/import_maps))
   with the following contents:

```json
{
  "imports": {
    "slint-ui": "npm:slint-ui"
  }
}
```

3. Create a new file called `index.ts` with the following contents:

```ts
import * as slint from "slint-ui";
let ui = slint.loadFile(new URL("main.slint", import.meta.url));
let demo = new ui.Demo();

await demo.run();
```

This is your main JavaScript entry point:

* Import the Slint API as an [ECMAScript module](https://nodejs.org/api/esm.html#modules-ecmascript-modules) module through Deno's
  NPM compatibility layer.
* Invoke `loadFile()` to compile and load the `.slint` file.
* Instantiate the `Demo` component declared in `main.slint`.
* Run it by showing it on the screen and reacting to user input.

1. Run the example with `deno run --allow-read --allow-ffi --allow-sys index.ts`


## Getting Started (bun)

1. In a new directory, create a new `bun` project by calling [`bun init`](https://bun.sh/docs/cli/init).
2. Install Slint for your project using [`bun install slint-ui`](https://bun.sh/docs/cli/install).
3. Create a new file called `main.slint` with the following contents:

```
import { AboutSlint, Button, VerticalBox } from "std-widgets.slint";
export component Demo inherits Window {
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

4. Clear the content of `index.ts` and add the following code:

```ts
import * as slint from "slint-ui";
let ui = slint.loadFile(new URL("main.slint", import.meta.url)) as any;
let demo = new ui.Demo();

await demo.run();
```

This is your main TypeScript entry point:

* Import the Slint API as an [ECMAScript module](https://nodejs.org/api/esm.html#modules-ecmascript-modules) module.
* Invoke `loadFile()` to compile and load the `.slint` file.
* Instantiate the `Demo` component declared in `main.slint`.
* Run it by showing it on the screen and reacting to user input.

5. Run the example with `bun run index.ts`


## API Overview

### Instantiating a Component

Use the {@link loadFile} function to load a `.slint` file. Instantiate the [exported component](http://slint.dev/docs/slint/guide/language/coding/file/)
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

**`main.mjs`**

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

### Accessing a Properties

[Properties](http://slint.dev/docs/slint/guide/language/coding/properties/) declared as `out` or `in-out` in `.slint` files are visible as JavaScript properties on the component instance.

**`main.slint`**
export component MainWindow {
    in-out property <string> name;
    in-out property <int> age: 42;
}

```js
let ui = slint.loadFile(new URL("main.slint", import.meta.url));
let instance = new ui.MainWindow();
console.log(instance.age); // Prints 42
instance.name = "Joe";
```

### Setting and Invoking Callbacks

[Callbacks](http://slint.dev/docs/slint/guide/language/coding/functions-and-callbacks/) declared in `.slint` files are visible as JavaScript function properties on the component instance. Invoke them
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

**`main.mjs`**

```js
import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("ui/my-component.slint", import.meta.url));
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
| array | {@link Model} | |

### Arrays and Models

[Array properties](http://slint.dev/docs/slint/guide/language/coding/repetition-and-data-models#arrays-and-models) can be set from JavaScript by passing
either `Array` objects or implementations of the {@link Model} interface.

When passing a JavaScript `Array` object, the contents of the array are copied. Any changes to the JavaScript afterwards will not be visible on the Slint side.

Reading a Slint array property from JavaScript will always return a @{link Model}.

```js
component.model = [1, 2, 3];
// component.model.push(4); // does not work, because assignment creates a copy.
// Use re-assignment instead.
component.model = component.model.concat(4);
```

Another option is to set an object that implements the {@link Model} interface.

### structs

An exported struct can be created either by defing of an object literal or by using the new keyword.

**`my-component.slint`**

```
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

```
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

### Globals

You can declare [globally available singletons](http://slint.dev/docs/slint/guide/language/coding/globals) in your
`.slint` files. If exported, these singletons are accessible as properties on your main
componen instance. Each global singleton is represented by an object with properties and callbacks,
similar to API that's created for your `.slint` component.

For example the following `.slint` markup defines a global `Logic` singleton that's also exported:

```
export global Logic {
    callback to_uppercase(string) -> string;
}
```

Assuming this global is used together with the `MyComponent` from the
previous section, you can access `Logic` like this:

```js
import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("ui/my-component.slint", import.meta.url));
let component = new ui.MyComponent();

component.Logic.to_upper_case = (str) => {
    return str.toUpperCase();
};
```

**Note**: Global singletons are instantiated once per component. When declaring multiple components for `export` to JavaScript,
each instance will have their own instance of associated globals singletons.

## Third-Party Licenses

For a list of the third-party licenses of all dependencies, see the separate [Third-Party Licenses page](thirdparty.html).
