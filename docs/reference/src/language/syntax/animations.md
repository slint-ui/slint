<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Animations

Declare animations for properties with the `animate` keyword like this:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    background: area.pressed ? blue : red;
    animate background {
        duration: 250ms;
    }

    area := TouchArea {}
}
```

This will animate the color property for 250ms whenever it changes.

Fine-tune animations using the following parameters:

-   `delay`: the amount of time to wait before starting the animation
-   `duration`: the amount of time it takes for the animation to complete
-   `iteration-count`: The number of times an animation should run. A negative value specifies
    infinite reruns. Fractual values are possible.
    For permanently running animations, see [`animation-tick()`](../builtins/functions.md#animation-tick-duration).
-   `easing`: can be any of the following. See [`easings.net`](https://easings.net/) for a visual reference:

    -   `linear`
    -   `ease-in-quad`
    -   `ease-out-quad`
    -   `ease-in-out-quad`
    -   `ease`
    -   `ease-in`
    -   `ease-out`
    -   `ease-in-out`
    -   `ease-in-quart`
    -   `ease-out-quart`
    -   `ease-in-out-quart`
    -   `ease-in-quint`
    -   `ease-out-quint`
    -   `ease-in-out-quint`
    -   `ease-in-expo`
    -   `ease-out-expo`
    -   `ease-in-out-expo`
    -   `ease-in-sine`
    -   `ease-out-sine`
    -   `ease-in-out-sine`
    -   `ease-in-back`
    -   `ease-out-back`
    -   `ease-in-out-back`
    -   `ease-in-circ`
    -   `ease-out-circ`
    -   `ease-in-out-circ`
    -   `ease-in-elastic`
    -   `ease-out-elastic`
    -   `ease-in-out-elastic`
    -   `ease-in-bounce`
    -   `ease-out-bounce`
    -   `ease-in-out-bounce`
    -   `cubic-bezier(a, b, c, d)` as in CSS

    Easing examples can also be found on the `Easings` tab of the `gallery` example.

It's also possible to animate several properties with the same animation, so:

```slint,ignore
animate x, y { duration: 100ms; easing: ease-out-bounce; }
```

is the same as:

```slint,ignore
animate x { duration: 100ms; easing: ease-out-bounce; }
animate y { duration: 100ms; easing: ease-out-bounce; }
```
