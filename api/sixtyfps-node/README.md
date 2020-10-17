# SixtyFPS-node

[SixtyFPS](https://www.sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS-node is the integration with node.

## Tutorial

```js
require("sixtyfps");
let ui = require("../ui/main.60");
let main = new ui.Main();
main.show();
```

## Example:

See [/examples/printerdemo/node](/examples/printerdemo/node)

## Documentation

By importing the sixtyfps module (or using require), a hook is installed that allows you
to import `.60` files directly.

```js
let sixtyfps = require("sixtyfps");
let ui = require("../ui/main.60");
```

### Instantiating a component

The exported component is exposed as a type constructor. The type constructor takes as parametter
an object which allow to initialize the value of public properties or signals.

```js
require("sixtyfps");
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

### types mapping

| `.60` Type | JS typr | Note |
| --- | --- | --- |
| `int` | Number | |
| `float` | Number | |
| `string` | String | |
| `color` |String | When reading a property, it is of the form `"#aarrggbb"`. When setting a property, any CSS complient color is accepted |
| `length` | Number |  |
| `logical_length` | Number | |
| `duration` | Number |  |
| structure | Object | With the properties being the same as the structure |
| array | Array or Model Object | |

### Model

For property of array type, they can either be set using an array.
In that case, getting the property also return an array.
If the array was set within the .60 file, the array can be obtained

```js
component.model = [1, 2, 3];
// component.model.push(4); // does not work, because it operate on a copy
// but re-assigning work
component.model = component.model.concat(4);
```

Another option is to set a model object.  A model object has the following function:
 - `row_count()`: returns the number of element in the model.
 - `row_data(index)`: return the row at the given index
 - `set_row_data(index, data)`: called when the model need to be changed. `this.notify.row_data_changed` must be called if successful.

 When such an object is set to a model property, it gets a new `notify` object with the following function
 - `row_data_changed(index)`: notify the view that the row was changed.
 - `row_added(index, count)`: notify the view that rows were added.
 - `row_removed(index, count)`: notify the view that a row were removed.

 As an example, here is the implementation of the `ArrayModel` (which is available on `sixtyfps.ArrayModel`)

 ```js
 let array = [1, 2, 3];
 let model = {
    row_count() { return a.length; },
    row_data(row) { return a[row]; },
    set_row_data(row, data) { a[row] = data; this.notify.row_data_changed(row); },
    push() {
        let size = a.length;
        Array.prototype.push.apply(a, arguments);
        this.notify.row_added(size, arguments.length);
    },
    remove(index, size) {
        let r = a.splice(index, size);
        this.notify.row_removed(size, arguments.length);
    },
};
component.model = model;
model.push(4); // this works
// does NOT work, getting the model does not return the right object
// component.model.push(5);
 ```

