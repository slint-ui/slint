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
Avoid using this callback, unless you need it, for example, in order to notify some native code:

```slint,no-preview
global SystemService  {
    // This callback can be implemented in native code using the Slint API
    callback ensure_service_running();
}

export component MySystemButton inherits Rectangle {
    init => {
        SystemService.ensure_service_running();
    }
    // ...
}
```
