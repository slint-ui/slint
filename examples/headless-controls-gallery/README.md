# Headless Controls Gallery

Styled buttons, selection controls, and sliders built on experimental headless controls.
The styled components delegate interaction through interfaces.

The gallery imports the shared headless `Button` as `ButtonBase`.
This base implements `ButtonInterface` and provides interaction and default accessibility.
Styled buttons inherit the base, including its accessibility properties and default action.
`GalleryButton` supplies its appearance through two named slots: `background` and `content`.
The base places content above the background without layout wrappers.
The supplied content uses a horizontal layout for padding and text placement.
See [the base](../../internal/compiler/widgets/headless/button.slint) and [the styled button](components/button.slint).

Run from the repository root:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 cargo run --bin slint-viewer -- --auto-reload examples/headless-controls-gallery/gallery.slint
```
