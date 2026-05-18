---
title: Slint for Node.js (Beta)
description: Use Slint from Node.js, Deno, or Bun — install slint-ui, load .slint files, and run native UI windows.
---

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

## API reference

The **API** group in the sidebar lists types and functions generated from the TypeScript sources in this repository.
Before you run `pnpm -C docs/nodejs run dev` or `pnpm -C docs/nodejs run build`, compile the native module and declarations from the repo root:

```sh
pnpm -C api/node run build
```

## Prerequisites

To use Slint with Node.js, ensure the following programs are installed:

  * **[Node.js](https://nodejs.org/download/release/)** (v24 or newer)
  * **[npm](https://www.npmjs.com/)**

To use Slint with Deno, ensure the following programs are installed:

  * **[Deno](https://docs.deno.com/runtime/manual)**

### Building from Source

Slint-node comes with pre-built binaries for macOS, Linux, and Windows. If you'd like to use Slint-node on a system
without pre-built binaries, you need to install additional software:

  * **[Rust compiler](https://www.rust-lang.org/tools/install)**
  * Depending on your operating system, you may need additional components. For a list of required system libraries,
    see <https://github.com/slint-ui/slint/blob/master/docs/building.md#prerequisites>.

## Getting Started (Node.js)

1. In a new directory, create a new Node.js project by calling [`npm init`](https://docs.npmjs.com/cli/v10/commands/npm-init).
2. Install Slint for your project using [`npm install slint-ui`](https://docs.npmjs.com/cli/v10/commands/npm-install).
3. Create a new file called `main.slint` with the following contents:

```slint playground
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

```slint playground
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

```slint playground
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

### Loading `.slint` Files

There are two ways to load a `.slint` file.
Both compile the `.slint` markup at runtime and return constructor functions
for each exported component.

#### Option 1: `import` with the Loader Hook (Recommended)

Import `.slint` files directly as ES modules using the Slint loader hook:

```js
import * as slint from "slint-ui";
import { MainWindow } from "./ui/main.slint";

let component = new MainWindow({
    counter: 42,
    clicked: function() { console.log("hello"); }
});
```

Register the hook when starting your app:

```sh
node --import slint-ui/register app.mjs
```

This is the recommended approach because:
- Imports are declarative and statically analyzable
- Works with TypeScript type checking (see [TypeScript Support](#typescript-support) below)
- Components, structs, and enums are available as named exports

You still import `slint-ui` for runtime helpers like `ArrayModel` and `runEventLoop`.

#### Option 2: `loadFile()`

Call `loadFile()` to compile and load a `.slint` file at runtime:

```js
import * as slint from "slint-ui";

let ui = slint.loadFile(new URL("ui/main.slint", import.meta.url));
let component = new ui.MainWindow({
    counter: 42,
    clicked: function() { console.log("hello"); }
});
```

Use this when you need to:
- Load `.slint` files dynamically (e.g. based on user input or configuration)
- Pass compiler options like `style`, `includePaths`, or `libraryPaths`
- Work without the `--import` flag (e.g. in environments that don't support loader hooks)

#### Instantiating a Component

With both approaches,
the exported component is available as a constructor function.
The constructor takes an optional object to set initial property values and callbacks.
The returned instance implements the `ComponentHandle` interface
for showing, hiding, and accessing the window.

Given this `.slint` file:

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

Instantiate it:

```js
let component = new MainWindow({
    counter: 42,
    clicked: function() { console.log("hello"); }
});
```

### Accessing Properties

[Properties](http://slint.dev/docs/slint/guide/language/coding/properties/) declared as `out` or `in-out` in `.slint` files are visible as JavaScript properties on the component instance.

**`main.slint`**

```slint
export component MainWindow {
    in-out property <string> name;
    in-out property <int> age: 42;
}
```

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

```slint
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

The types used for properties in .slint design markup each translate to specific types in JavaScript. The following table summarizes the entire mapping:

| `.slint` Type | JavaScript Type | Notes |
| --- | --- | --- |
| `int` | `Number` | |
| `bool` | `Boolean` | |
| `float` | `Number` | |
| `string` | `String` | |
| `color` | `RgbaColor` | |
| `brush` | `Brush` | |
| `image` | `ImageData` | |
| `length` | `Number` | |
| `physical_length` | `Number` | |
| `duration` | `Number` | The number of milliseconds |
| `angle` | `Number` | The angle in degrees |
| `relative-font-size` | `Number` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
| structure | `Object` | Structures are mapped to JavaScript objects where each structure field is a property. |
| array | `Model` | |

### Arrays and Models

[Array properties](http://slint.dev/docs/slint/guide/language/coding/repetition-and-data-models#arrays-and-models) can be set from JavaScript by passing
either `Array` objects or implementations of the `Model` interface.

When passing a JavaScript `Array` object, the contents of the array are copied. Any changes to the JavaScript afterwards will not be visible on the Slint side.

Reading a Slint array property from JavaScript will always return a `Model`.

```js
component.model = [1, 2, 3];
// component.model.push(4); // does not work, because assignment creates a copy.
// Use re-assignment instead.
component.model = component.model.concat(4);
```

Another option is to set an object that implements the `Model` interface.

### structs

An exported struct can be created either by defining an object literal or by using the new keyword.

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

### Globals

You can declare [globally available singletons](http://slint.dev/docs/slint/guide/language/coding/globals) in your
`.slint` files. If exported, these singletons are accessible as properties on your main
component instance. Each global singleton is represented by an object with properties and callbacks,
similar to API that's created for your `.slint` component.

For example the following `.slint` markup defines a global `Logic` singleton that's also exported:

```slint
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

**Note**: Global singletons are instantiated once per component.
When declaring multiple components for `export` to JavaScript,
each instance will have their own instance of associated global singletons.

### TypeScript Support

The loader hook works with TypeScript out of the box — `.slint` files compile and run.
But TypeScript doesn't know the types of the exported components until you generate a declaration file.

#### Generating Type Declarations

Use `slint-compiler` to generate a `.d.ts` file:

```sh
slint-compiler -f typescript ui/main.slint -o ui/main.slint.d.ts
```

This gives you full IDE autocomplete and type checking for all properties,
callbacks, structs, and enums exported from the `.slint` file.

Wire it into `package.json` so types stay in sync:

```json
{
  "scripts": {
    "generate": "slint-compiler -f typescript ui/main.slint -o ui/main.slint.d.ts",
    "start": "npm run generate && node --import slint-ui/register app.mjs"
  }
}
```

Add `*.slint.d.ts` to `.gitignore` — the generated file is a build artifact.
The app runs without it; you just lose type checking.

#### tsconfig Setup

Use `moduleResolution: "bundler"` so TypeScript resolves `import "./main.slint"`
to the generated `main.slint.d.ts`:

```json
{
  "compilerOptions": {
    "module": "esnext",
    "moduleResolution": "bundler"
  }
}
```

For a complete example,
see [/examples/todo/node-typescript](https://github.com/slint-ui/slint/tree/master/examples/todo/node-typescript).

## Third-Party Licenses

For a list of the third-party licenses of all dependencies, see the separate [Third-Party Licenses page](/thirdparty/).
