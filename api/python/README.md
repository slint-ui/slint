<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint-python (Beta)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-python is the integration with Python.

**Warning**
Slint-python is in a beta phase of development: The APIs while mostly stable, may be subject to further changes. Any changes will be documented in the ChangeLog.

You can track the progress for the Python integration by looking at python-labelled issues at https://github.com/slint-ui/slint/labels/a%3Alanguage-python .

## Slint Language Manual

The [Slint Language Documentation](../slint) covers the Slint UI description language
in detail.

## Prerequisites

 * [Python 3](https://python.org/)
 * [uv](https://docs.astral.sh/uv/) or [pip](https://pypi.org/project/pip/)

## Installation

Slint can be installed with `uv` or `pip` from the [Python Package Index](https://pypi.org):

```bash
uv add slint
```

The installation uses binaries provided for macOS, Windows, and Linux for various architectures. If your target platform
is not covered by binaries, `uv` will automatically build Slint from source. If that happens, you will then need some
software development tools on your machine, as well as [Rust](https://www.rust-lang.org/learn/get-started).

## Quick Start

1. Create a new project with `uv init`.
2. Add the Slint Python package to your Python project: `uv add slint`
3. Create a file called `app-window.slint`:

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

4. Create a file called `main.py`:

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

5. Run it with `uv run main.py`

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

The exported component is exposed as a Python class. To access this class, you have two options:

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

   Any attribute lookup in `slint.loader` is searched for in `sys.path`. If a directory with the name exists, it is
   returned as a loader object, and subsequent attribute lookups follow the same logic.

   If the name matches a file with the `.slint` extension, it is automatically loaded with `load_file` and the
   [namespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace) is returned.

   If the file name contains a dash, like `app-window.slint`, an attribute lookup for `app_window` tries to
   locate `app_window.slint` and then fall back to `app-window.slint`.

### Accessing Properties

[Properties](../slint/src/language/syntax/properties) declared as `out` or `in-out` in `.slint` files are visible as
properties on the component instance.

```python
main_window.counter = 42
print(main_window.counter)
```

### Accessing Globals

[Global Singletons](https://slint.dev/docs/slint/src/language/syntax/globals#global-singletons) are accessible in
Python as properties in the component instance.

For example, this Slint code declares a `PrinterJobQueue` singleton:

```slint
export global PrinterJobQueue {
    in-out property <int> job-count;
}
```

Access it as a property on the component instance by its name:

```python
print("job count:", instance.PrinterJobQueue.job_count)
```

**Note**: Global singletons are instantiated once per component. When declaring multiple components for `export` to Python,
each instance has their own associated globals singletons.

### Setting and Invoking Callbacks

[Callbacks](src/language/syntax/callbacks) declared in `.slint` files are visible as callable properties on the component
instance. Invoke them as functions to invoke the callback, and assign Python callables to set the callback handler.

In Slint, callbacks are defined using the `callback` keyword and can be connected to another component's callback using
the `<=>` syntax.

**`my-component.slint`**

```slint
export component MyComponent inherits Window {
    callback clicked <=> i-touch-area.clicked;

    width: 400px;
    height: 200px;

    i-touch-area := TouchArea {}
}
```

The callbacks in Slint are exposed as properties and that can be called as functions.

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

The `@slint.callback()` decorator accepts a `name` argument, if the name of the method does not match the name of the
callback in the `.slint` file. Similarly, a `global_name` argument can be used to bind a method to a callback in a global
singleton.

### Type Mappings

Each type used for properties in the Slint Language translates to a specific type in Python. The following table summarizes
the mapping:

| `.slint` Type | Python Type | Notes |
| ------------- | ----------- | ----- |
| `int`         | `int`       |       |
| `float`       | `float`     |       |
| `string`      | `str`       |       |
| `color`       | `slint.Color` |     |
| `brush`       | `slint.Brush` |     |
| `image`       | `slint.Image` |     |
| `length`      | `float`     |       |
| `physical_length` | `float` |       |
| `duration`    | `float`     | The number of milliseconds |
| `angle`       | `float`     | The angle in degrees |
| structure     | `dict`/`Struct` | When reading, structures are mapped to data classes, when writing dicts are also accepted. |
| array         | `slint.Model` |     |

### Arrays and Models

You can set [array properties](../slint/src/language/syntax/types#arrays-and-models) from Python by passing subclasses of
`slint.Model`.

Use the `slint.ListModel` class to construct a model from an iterable:

```python
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

When adding or inserting rows, call `notify_row_added(row, count)` on the super class. Similarly, when removing rows, notify
Slint by calling `notify_row_removed(row, count)`.

### Structs

Structs declared in Slint and exposed to Python via `export` are then accessible in the namespace that is returned
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

The exported `MyData` struct can be constructed as follows:

```python
import slint
# Look for for `app.slint` in `sys.path`:
main_window = slint.loader.app.MainWindow()

data = slint.loader.app.MyData(name = "Simon")
data.age = 10
main_window.data = data
```

## Third-Party Licenses

For a list of the third-party licenses of all dependencies, see the separate [Third-Party Licenses page](thirdparty.html).
