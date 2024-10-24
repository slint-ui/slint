<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint-python (Alpha)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-python is the integration with Python.

**Warning: Alpha**
Slint-python is still in the very early stages of development: APIs will change and important features are still being developed,
the project is overall incomplete.

You can track the overall progress for the Python integration by looking at python-labelled issues at https://github.com/slint-ui/slint/labels/a%3Alanguage-python .

## Slint Language Manual

The [Slint Language Documentation](../slint) covers the Slint UI description language
in detail.

## Prerequisites

 * [Python 3](https://python.org/)
 * [pip](https://pypi.org/project/pip/)
 * [Pipenv](https://pipenv.pypa.io/en/latest/installation.html#installing-pipenv)

## Installation

Slint can be installed with `pip` from the [Python Package Index](https://pypi.org):

```
pip install slint
```

The installation will use binaries provided vi macOS, Windows, and Linux for various architectures. If your target platform is not covered by binaries,
`pip` will automatically build Slint from source. If that happens, you need common software development tools on your machine, as well as [Rust](https://www.rust-lang.org/learn/get-started).

### Building from Source

## Try it out

If you want to just play with this, you can try running our Python port of the [printer demo](../../demos/printerdemo/python/README.md):

```bash
cd demos/printerdemo/python
pipenv update
pipenv run python main.py
```

## Quick Start

1. Add Slint Python Package Index to your Python project: `pipenv install slint`
2. Create a file called `app-window.slint`:

```slint
import { Button, VerticalBox } from "std-widgets.slint";

export component AppWindow inherits Window {
    in-out property<int> counter: 42;
    callback request-increase-value();
    VerticalBox {
        Text {
            text: "Counter: \{root.counter}";
        }
        Button {
            text: "Increase value";
            clicked => {
                root.request-increase-value();
            }
        }
    }
}
```

1. Create a file called `main.py`:

```python
import slint

# slint.loader will look in `sys.path` for `app-window.slint`.
class App(slint.loader.app_window.AppWindow):
    @slint.callback
    def request_increase_value(self):
        self.counter = self.counter + 1

app = App()
app.run()
```

4. Run it with `pipenv run python main.py`

## API Overview

### Instantiating a Component

The following example shows how to instantiate a Slint component in Python:

**`app.slint`**

```slint
export component MainWindow inherits Window {
    callback clicked <=> i-touch-area.clicked;

    in property <int> counter;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The exported component is exposed as a Python class. To access this class, you have two
options:

1. Call `slint.load_file("app.slint")`. The returned object is a [namespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace),
   that provides the `MainWindow` class as well as any other explicitly exported component that inherits `Window`:
   ```python
   import slint
   components = slint.load_file("app.slint")
   main_window = components.MainWindow()
   ```

2. Use Slint's auto-loader, which lazily loads `.slint` files from `sys.path`:
   ```python
   import slint
   # Look for for `app.slint` in `sys.path`:
   main_window = slint.loader.app.MainWindow()
   ```

   Any attribute lookup in `slint.loader` is searched for in `sys.path`. If a directory with the name exists, it is returned as a loader object, and subsequent
   attribute lookups follow the same logic. If the name matches a file with the `.slint` extension, it is automatically loaded with `load_file` and the
   [namespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace) is returned, which contains classes for each exported component that
   inherits `Window`. If the file name contains a dash, like `app-window.slint`, an attribute lookup for `app_window` will
   first try to locate `app_window.slint` and then fall back to `app-window.slint`.

### Accessing Properties

[Properties](../slint/src/language/syntax/properties) declared as `out` or `in-out` in `.slint` files are visible as  properties on the component instance.

```python
main_window.counter = 42
print(main_window.counter)
```

### Accessing Globals

[Global Singletons](https://slint.dev/docs/slint/src/language/syntax/globals#global-singletons) are accessible in
Python as properties in the component instance:

```slint,ignore
export global PrinterJobQueue {
    in-out property <int> job-count;
}
```

```python
print("job count:", instance.PrinterJobQueue.job_count)
```

**Note**: Global singletons are instantiated once per component. When declaring multiple components for `export` to Python,
each instance will have their own instance of associated globals singletons.

### Setting and Invoking Callbacks

[Callbacks](src/language/syntax/callbacks) declared in `.slint` files are visible as callable properties on the component instance. Invoke them
as function to invoke the callback, and assign Python callables to set the callback handler.

Callbacks in Slint can be defined using the `callback` keyword and can be connected to a callback of an other component
using the `<=>` syntax.

**`my-component.slint`**

```slint
export component MyComponent inherits Window {
    callback clicked <=> i-touch-area.clicked;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The callbacks in Slint are exposed as properties and that can be called as a function.

**`main.py`**

```python
import slint

component = slint.loader.my_component.MyComponent()
# connect to a callback

def clicked():
    print("hello")

component.clicked = clicked
// invoke a callback
component.clicked();
```

Another way to set callbacks is to sub-class and use the `@slint.callback` decorator:

```python
import slint

class Component(slint.loader.my_component.MyComponent):
    @slint.callback
    def clicked(self):
        print("hello")

component = Component()
```

The `@slint.callback()` decorator accepts a `name` named argument, when the name of the method
does not match the name of the callback in the `.slint` file. Similarly, a `global_name` argument
can be used to bind a method to a callback in a global singleton.

### Type Mappings

The types used for properties in the Slint Language each translate to specific types in Python. The follow table summarizes the entire mapping:

| `.slint` Type | Python Type | Notes |
| --- | --- | --- |
| `int` | `int` | |
| `float` | `float` | |
| `string` | `str` | |
| `color` | `slint.Color` |  |
| `brush` | `slint.Brush` |  |
| `image` | `slint.Image` |  |
| `length` | `float` | |
| `physical_length` | `float` | |
| `duration` | `float` | The number of milliseconds |
| `angle` | `float` | The angle in degrees |
| structure | `dict`/`Struct` | When reading, structures are mapped to data classes, when writing dicts are also accepted. |
| array | `slint.Model` | |

### Arrays and Models

[Array properties](../slint/src/language/syntax/types#arrays-and-models) can be set from Python by passing
subclasses of `slint.Model`.

Use the `slint.ListModel` class to construct a model from an iterable.

```js
component.model = slint.ListModel([1, 2, 3]);
component.model.append(4)
del component.model[0]
```

When sub-classing `slint.Model`, provide the following methods:

```python
    def row_count(self):
        """Return the number of rows in your model"""

    def row_data(self, row):
        """Return data at specified row"""

    def set_row_data(self, row, data):
        """For read-write models, store data in the given row. When done call set.notify_row_changed:"
        ..."""
        self.notify_row_changed(row)
```

When adding/inserting rows, call `notify_row_added(row, count)` on the super class. Similarly, removal
requires notifying Slint by calling `notify_row_removed(row, count)`.

### Structs

Structs declared in Slint and exposed to Python via `export` are accessible in the namespace returned
when [instantiating a component](#instantiating-a-component).

**`app.slint`**

```slint
export struct MyData {
    name: string,
    age: int
}

export component MainWindow inherits Window {
    in-out property <MyData> data;
}
```

**`main.py`**

The exported `MyData` struct can be constructed

```python
import slint
# Look for for `app.slint` in `sys.path`:
main_window = slint.loader.app.MainWindow()

data = slint.loader.app.MyData(name = "Simon")
data.age = 10
main_window.data = data
```
