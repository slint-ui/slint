# Callback

Components may declare callbacks, that communicate changes of state
to the outside. Callbacks are emitted by "calling" them like you would
call a function.

You react to callback emissions by declaring a handler using the `=>` arrow syntax.
The built-in `TouchArea` element comes with a `clicked` callback, that's emitted
when the user touches the rectangular area covered by the element, or clicks into
it with the mouse. In the example below, the emission of that callback is forwarded
to another custom callback (`hello`) by declaring a handler and emitting our
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

It's possible to add parameters to a callback;

```slint,no-preview
export component Example inherits Rectangle {
    // declares a callback
    callback hello(int, string);
    hello(aa, bb) => { /* ... */ }
}
```

Callbacks may also return a value.

```slint,no-preview
export component Example inherits Rectangle {
    // declares a callback with a return value
    callback hello(int, int) -> int;
    hello(aa, bb) => { aa + bb }
}
```
