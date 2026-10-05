# Slot Examples

## Straight and Circular Sliders

[The slider example](sliders.slint) plugs two interaction surfaces into the same headless slider.
`HeadlessSlider` owns range limits, step rounding, keyboard controls, accessibility actions, and change callbacks.
Its typed `surface` slot requires the `SliderSurface` interface: enabled state, normalized position, focus state, and a `seek` callback.

`StraightSurface` converts horizontal pointer position into a requested position.
`CircularSurface` converts pointer angle along a 270-degree arc, starting at the bottom left and ending at the bottom right.
Requests in the bottom gap clamp to the nearest endpoint; the center ignores pointer input because it has no useful angle.
The headless slider converts requests into values, clamps them, and rounds them to the configured step.
A zero or negative step disables rounding; equal or reversed bounds collapse to the minimum.
External value assignments are displayed with a clamped position but aren't rewritten or reported as user changes.

Both sliders bind to the same external value, so interacting with either updates both.
Each surface also has an `accent` property outside the shared interface, which callers can customize.
Click or drag a slider, then use the arrow keys, Home, or End.

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 cargo run --bin slint-viewer -- examples/slots/sliders.slint
```

Use a checkout that includes typed-slot support.
The feature is experimental.

See the [Named Slots guide](../../docs/astro/src/content/docs/guide/experimental/named-slots.mdx#typed-slots) for the syntax and interface contract.
