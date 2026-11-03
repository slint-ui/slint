# Tab Design Comparison

The two branches use the same controls library and gallery to compare tab APIs.

| | Nested declarations | Separate bar and panels |
| --- | --- | --- |
| Branch | `ionuc/tab-header-slot` | `ionuc/composable-tabs` |
| Example | `examples/tabs.slint` | `examples/tabs.slint` |
| Header defaults | Supplied explicitly for Basic headers | Basic `TabBar` creates Basic headers |
| Page association | Compiler-generated indexes and bindings | Explicit `tab-index` and `current-index` bindings |
| Custom headers | Per-page `header` slot, with a custom `bar` | Header components inside `TabBarLayout` |
| Runtime orientation | Unsupported | Supported by the bar and application layout |
| Repeated or conditional tabs | `for` and `if` on `Tab` are unsupported | Ordinary repeaters and conditionals; the application keeps headers and panels aligned |
| Compiler changes | Tab slot validation, header relocation, generated bindings, and inheritance handling | No tab-specific compiler changes |

## Run the Examples

Check out either branch, then run:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "interfaces,shadowable,closures,named-slots"
cargo run -p slint-viewer --bin slint-viewer --no-default-features --features backend-winit,renderer-software -- --auto-reload tools/editor/controls/examples/tabs.slint
```

Rebuilding through `cargo run` avoids using a viewer with widget definitions embedded from another branch.
The examples also appear on the gallery's Tabs page in `examples/controls.slint`.

## Nested Declaration Limitations

`TabWidget` requires a static list of `Tab` children.
Its orientation is constant: horizontal places the bar above the page, and vertical places it on the left.
A subclass that already declares pages cannot accept more pages or an orientation override at its use site.
Custom headers require an explicit custom bar.
Replacing only the bar retains the standard style's default headers.
`Tab` has no per-page `enabled` property or built-in closing and reordering support.

All pages are created and kept alive; switching selection changes visibility.
Hidden pages retain their state and can continue running callbacks and timers.
In the existing Fluent style, keyboard navigation can leave the selected header outside the visible viewport.

The compiler connects title, selection, focus, activation, accessibility, and geometry.
The branch preserves application activation handlers and lowers widgets copied from inherited custom bars.

## Separate Bar and Panel Tradeoffs

The application declares a bar and its panels separately and connects them through `current-index`.
The Basic `TabBar` generates headers from its `tabs` model.
Use `TabBarLayout` when supplying custom headers and bind their selection, focus, and activation explicitly.

Indexes identify positions, not stable page identities.
The application keeps the model and panels in the same order and chooses what happens when items are inserted or removed.
Ordinary conditional elements are created and destroyed as their conditions change.
Keeping an unselected panel instantiated preserves its page state.

Keyboard navigation and selection belong to the bar; visual slots belong to individual headers.
Scrolling an offscreen focused header into view remains follow-up work for the shared bar layout.
The original standard `TabWidget` compiler pass and API remain unchanged on the separate-design branch.
