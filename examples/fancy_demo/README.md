# Fancy Demo

A showcase of custom widget implementations built entirely in Slint without using the built-in widget library.

## What It Demonstrates

This example implements a complete widget set from scratch, demonstrating how to build custom UI components using Slint's primitives:

### Custom Widgets

- **MdiWindow** - Draggable, collapsible MDI-style windows with close buttons
- **Button** - Custom styled buttons with hover effects
- **CheckBox** - Animated checkbox with custom graphics
- **RadioButton** - Radio button implementation
- **SelectableLabel** - Clickable label that acts like a radio button
- **Slider** - Custom slider with track and handle
- **Hyperlink** - Clickable text that opens URLs
- **DragValue** - Numeric input that can be adjusted by dragging
- **ProgressBar** - Animated progress indicator
- **LineEdit** - Text input field (uses built-in TextInput)

### UI Patterns

- MDI (Multiple Document Interface) window management
- Custom theming via a `Palette` global
- Touch/mouse interaction handling
- Animations and state transitions
- Path-based vector graphics for icons

## Running

```sh
cargo run -p fancy_demo
```

Or with the viewer:

```sh
cargo run --bin slint-viewer -- examples/fancy_demo/main.slint
```
