# Callbacks

Components may declare callbacks, that communicate changes of state
to the outside. Callbacks are invoked by "calling" them like you would
call a function.

You react to callback invocation by declaring a handler using the `=>` arrow syntax.
The built-in `TouchArea` element declares a `clicked` callback, that's invoked
when the user touches the rectangular area covered by the element, or clicks into
it with the mouse. In the example below, the invocation of that callback is forwarded
to another custom callback (`hello`) by declaring a handler and invoking our
custom callback:

```slint,no-preview
export component Example inherits Rectangle {
    // declare a callback
    callback hello;

    area := TouchArea {
        // sets a handler with `=>`
        clicked => {
            // emit the callback
            root.hello()
        }
    }
}
```

It's possible to add parameters to a callback:

```slint,no-preview
export component Example inherits Rectangle {
    // declares a callback
    callback hello(int, string);
    hello(aa, bb) => { /* ... */ }
}
```

Callbacks may also return a value:

```slint,no-preview
export component Example inherits Rectangle {
    // declares a callback with a return value
    callback hello(int, int) -> int;
    hello(aa, bb) => { aa + bb }
}
```

## Callback aliases

It is possible to declare callback aliases in a similar way to two-way bindings:

```slint,no-preview
export component Example inherits Rectangle {
    callback clicked <=> area.clicked;
    area := TouchArea {}
}
```
