# SixtyFPS-node

[SixtyFPS](/README.md) is a UI toolkit that supports different programming languages.
SixtyFPS-node is the integration with node.

## Tutorial

```js
require("sixtyfps");
let ui = require("../ui/main.60");
let main = new ui.Main();
main.show();
```

## Example:

See [/examples/nodetest](/examples/nodetest)

## Documentation

By importing the sixtyfps module (or using require), a hook is installed that allows you
to import `.60` files directly.

```js
let ui = require("../ui/main.60");
```

### Instantiating a component

The exported component is exposed as a type constructor. The type constructor takes as parametter
an object which allow to initialize the value of public properties or signals.

```js
// In this example, the main.60 file exports a module which
// has a counter property and a clicked signal
let ui = require("ui/main.60");
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

### Signals

The signals are also exposed as property that can be called

```js
// connect to a signal
component.clicked = function() { console.log("hello"); }
// emit a signal
component.clicked();
```
