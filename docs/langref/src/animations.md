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

This will animate the color property for 100ms whenever it changes.

Fine-tune animations using the following parameters:

-   `delay`: the amount of time to wait before starting the animation
-   `duration`: the amount of time it takes for the animation to complete
-   `iteration-count`: The number of times a animation should run. A negative value specifies
    infinite reruns. Fractual values are possible.
-   `easing`: can be `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(a, b, c, d)` as in CSS

It's also possible to animate several properties with the same animation:

```slint,ignore
animate x, y { duration: 100ms; }
```

is the same as

```slint,ignore
animate x { duration: 100ms; }
animate y { duration: 100ms; }
```
