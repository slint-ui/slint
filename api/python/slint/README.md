
# Slint-python (Beta)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-python is the integration with Python.

**Warning**
Slint-python is in a beta phase of development: the APIs, while mostly stable, may be subject to further changes. Any changes will be documented in the [ChangeLog](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md).

You can track the progress of the Python integration in the [python-labeled issues](https://github.com/slint-ui/slint/labels/a%3Alanguage-python).

## Slint Language Manual

The [Slint Language Documentation](https://docs.slint.dev/latest/docs/slint/) covers the Slint UI description language
in detail.

## Prerequisites

 * [Python 3.12 or newer](https://python.org/)
 * [uv](https://docs.astral.sh/uv/) or [pip](https://pypi.org/project/pip/)

## Installation

Install Slint with `uv` or `pip` from the [Python Package Index](https://pypi.org):

```bash
uv add slint
```

The installation uses binaries provided for macOS, Windows, and Linux for various architectures. If your target platform
isn't covered by binaries, `uv` automatically builds Slint from source. In that case, you need some
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


# slint.loader looks in `sys.path` for `app-window.slint`.
class App(slint.loader.app_window.AppWindow):
    @slint.callback
    def request_increase_value(self):
        self.counter = self.counter + 1


app = App()
app.run()
```

5. Run it with `uv run main.py`.

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

1. Call `slint.load_file("app.slint")`. The returned object is a [namespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace)
   that provides the `MainWindow` class as well as any other explicitly exported component that inherits `Window`:

   ```python
   import slint

   components = slint.load_file("app.slint")
   main_window = components.MainWindow()
   ```

2. Use Slint's auto-loader, which lazily loads `.slint` files from `sys.path`:

   ```python
   import slint

   # Look for `app.slint` in `sys.path`:
   main_window = slint.loader.app.MainWindow()
   ```

   `slint.loader` searches `sys.path` for any attribute you look up. If a directory with that name exists, the loader
   returns it as a loader object; subsequent attribute lookups follow the same logic.

   If the name matches a file with the `.slint` extension, the loader loads it with `load_file` and returns the
   [namespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace).

   If the file name contains a dash, like `app-window.slint`, an attribute lookup for `app_window` tries to
   locate `app_window.slint` and then falls back to `app-window.slint`.

### Accessing Properties

[Properties](https://docs.slint.dev/latest/docs/slint/reference/language/properties/) declared as `out` or `in-out` in `.slint` files are visible as
properties on the component instance.

```python
main_window.counter = 42
print(main_window.counter)
```

### Accessing Globals

You can access [global singletons](https://docs.slint.dev/latest/docs/slint/reference/language/globals/) in Python as properties of the component
instance.

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

**Note:** Global singletons are instantiated once per component. When you export multiple components to Python,
each instance has its own associated global singletons.

### Setting and Invoking Callbacks

[Callbacks](https://docs.slint.dev/latest/docs/slint/reference/language/callbacks/) declared in `.slint` files are visible as callable properties on the
component instance. Call them as functions to invoke the callback, and assign a Python callable to set the callback handler.

In Slint, you define callbacks with the `callback` keyword and connect them to another component's callback with
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

The callbacks are exposed as properties that you can call as functions:

**`main.py`**

```python
import slint

component = slint.loader.my_component.MyComponent()


# Connect to a callback.
def clicked():
    print("hello")


component.clicked = clicked
# Invoke a callback.
component.clicked()
```

Another way to set callbacks is to subclass the component and use the `@slint.callback` decorator:

```python
import slint


class Component(slint.loader.my_component.MyComponent):
    @slint.callback
    def clicked(self):
        print("hello")


component = Component()
```

If the name of your method doesn't match the name of the callback in the `.slint` file, pass a `name` argument to the
`@slint.callback()` decorator. Similarly, to bind a method to a callback in a global singleton, pass a `global_name`
argument.

### Type Mappings

Each type used for properties in the Slint language translates to a specific type in Python. See the
[type mappings table](https://docs.slint.dev/latest/docs/python/#type-mappings) in the Slint Python documentation for the
complete list.

### Arrays and Models

You can set [array properties](https://docs.slint.dev/latest/docs/slint/reference/property-types/arrays-and-models/) from Python by passing subclasses of
`slint.Model`.

Use the `slint.ListModel` class to construct a model from an iterable:

```python
component.model = slint.ListModel([1, 2, 3])
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
    """For read-write models, store data in the given row. When done, call notify_row_changed."""
    ...
    self.notify_row_changed(row)
```

When adding or inserting rows, call `notify_row_added(row, count)` on the superclass. Similarly, when removing rows, notify
Slint by calling `notify_row_removed(row, count)`.

### Structs

Structs that you declare and `export` in Slint are accessible in the namespace that's returned
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

data = slint.loader.app.MyData(name="Simon")
data.age = 10
main_window.data = data
```

### Enums

Enums that you declare and `export` in Slint are accessible in the namespace that's returned
when [instantiating a component](#instantiating-a-component). The enums are subclasses of [enum.Enum](https://docs.python.org/3/library/enum.html).

**`app.slint`**

```slint
export enum MyOption {
    Variant1,
    Variant2
}

export component MainWindow inherits Window {
    in-out property <MyOption> data;
}
```

**`main.py`**

Variants of the exported `MyOption` enum can be constructed as follows:

```python
import slint

# Look for for `app.slint` in `sys.path`:
main_window = slint.loader.app.MainWindow()

value = slint.loader.app.MyOption.Variant2
main_window.data = value
```

## Asynchronous I/O

Use Python's [asyncio](https://docs.python.org/3/library/asyncio.html) library to write concurrent Python code with the `async`/`await` syntax.

Slint's event loop is a full-featured [asyncio event loop](https://docs.python.org/3/library/asyncio-eventloop.html#asyncio-event-loop). While
the event loop is running, [`asyncio.get_running_loop()`](https://docs.python.org/3/library/asyncio-eventloop.html#asyncio.get_running_loop) returns
a valid loop. To run an async function when starting the loop, pass a coroutine to `slint.run_event_loop()`.

For the common use case of interacting with REST APIs, we recommend the [`aiohttp` library](https://docs.aiohttp.org/en/stable/).

### Known Limitations

- Pipes and sub-processes are only supported on Unix-like platforms.

## Type Hints

[PEP 484](https://peps.python.org/pep-0484/) introduces a standard syntax for type annotations to Python, enabling static analysis for
type checking, refactoring, and code completion. Popular type checkers include [mypy](https://mypy-lang.org/), [Pyre](https://pyre-check.org),
and Astral's [ty](https://docs.astral.sh/ty/).

Use Slint's [slint-compiler](https://pypi.org/project/slint-compiler/) to generate `.py` files with type annotations for your `.slint` files.
They replace calls to `load_file` and any use of `slint.loader`.

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

4. Run the [slint-compiler](https://pypi.org/project/slint-compiler/) to generate `app_window.py`:
    `uvx slint-compiler -f python -o app_window.py app-window.slint`

    If your `slint-project.json` sets `"main": "app-window.slint"`, pass the project directory instead:
    `uvx slint-compiler -f python -o app_window.py .`.
    The generated module then loads that project directory at run time, too.

5. Create a file called `main.py`:

```python
import slint
import app_window


class App(app_window.AppWindow):
    @slint.callback
    def request_increase_value(self):
        self.counter = self.counter + 1


app = App()
app.run()
```

6. Run it with `uv run main.py`.

## Third-Party Licenses

For a list of the third-party licenses of all dependencies, see the separate [Third-Party Licenses page](https://docs.slint.dev/latest/docs/python/thirdparty/).
