<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Builtin Callbacks

## `init()`

Every element implicitly declares an `init` callback. You can assign a code block to it that will be invoked when the
element is instantiated and after all properties are initialized with the value of their final binding. The order of
invocation is from inside to outside. The following example will print "first", then "second", and then "third":

```slint,no-preview
component MyButton inherits Rectangle {
    in-out property <string> text: "Initial";
    init => {
        // If `text` is queried here, it will have the value "Hello".
        debug("first");
    }
}

component MyCheckBox inherits Rectangle {
    init => { debug("second"); }
}

export component MyWindow inherits Window {
    MyButton {
        text: "Hello";
        init => { debug("third"); }
    }
    MyCheckBox {
    }
}
```

Don't use this callback to initialize properties, because this violates the declarative principle.

Even though the `init` callback exists on all components, it cannot be set from application code,
i.e. an `on_init` function does not exist in the generated code. This is because the callback is invoked during the creation of the component, before you could call `on_init` to actually set it.

While the `init` callback can invoke other callbacks, e.g. one defined in a `global` section, and
you _can_ bind these in the backend, this doesn't work for statically-created components, including
the window itself, because you need an instance to set the globals binding. But it is possible
to use this for dynamically created components (for example ones behind an `if`):

```slint,no-preview
export global SystemService  {
    // This callback can be implemented in native code using the Slint API
    callback ensure_service_running();
}

component MySystemButton inherits Rectangle {
    init => {
        SystemService.ensure_service_running();
    }
    // ...
}

export component AppWindow inherits Window {
    in property <bool> show-button: false;

    // MySystemButton isn't initialized at first, only when show-button is set to true.
    // At that point, its init callback will call ensure_service_running()
    if show-button : MySystemButton {}
}
```
