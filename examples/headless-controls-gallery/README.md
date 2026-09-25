# Headless Controls Gallery

Button and Slider examples built on shared experimental controls.
The styled components inherit behavior, interfaces, and accessibility from their bases.

`GalleryButton` supplies `background` and `content` slots.
Its content uses a horizontal layout for padding and text placement.
See [the button base](../../internal/compiler/widgets/headless/button.slint) and [the styled button](components/button.slint).

`GallerySlider` supplies `track` and `thumb` slots for a straight horizontal slider.
The style defines the thumb geometry and exposes its bounds through the base's `handle-*` properties.
The base handles pointer input, dragging, keyboard input, focus, and accessibility.
See [the slider base](../../internal/compiler/widgets/headless/slider.slint) and [the styled slider](components/slider.slint).

Run from the repository root:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 cargo run --bin slint-viewer -- --auto-reload examples/headless-controls-gallery/gallery.slint
```
