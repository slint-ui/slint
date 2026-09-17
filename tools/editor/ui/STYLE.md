# Editor style ownership

`style.slint` owns the editor's authored colors, brushes, typography, spacing, control metrics, layout metrics, shadows, and feedback durations.
Import `Style` directly and use a semantic field, such as `Style.inspector.field-height` or `Style.colors.action-bg`.
The eleven groups separate shared foundations from the inspector, navigation, canvas, and welcome compositions.

## Adding and changing values

Use an existing role when the appearance has the same purpose.
Add a field when a live consumer needs an independently adjustable design decision.
Name the purpose rather than the current number or color.
For example, use `slice-hit-size`, not `size-20`.

The spacing scale is 2, 4, 6, 8, 12, 16, and 24px.
Ordinary shell controls are 32px high; inspector controls, palette rows, and recent projects retain their distinct densities.
The new-file action retains its compact 28px target.
The collapsed-sidebar action group retains its separate 12px container radius.
Semantic fields derive from shared foundations where they represent the same decision.

Derive centering, inner widths, circle radii, and combined heights from their owning metrics.
Use the same metrics for rendering and pointer mapping.
Keep visual handle sizes separate from their larger hit areas.
The shared `EditorGrid` derives its coverage from the viewport and grid pitch.

Slint tracks dependencies at the struct-property level.
Reading another field of the same struct in its initializer creates a binding loop.
Use a private scalar source for fields that depend on a sibling, as the existing grouped definitions demonstrate.

Use `color` for text and color operations, and `brush` for decorative gradients or surfaces that need them.
Use `TextStyle` for text roles and `ShadowStyle` for elevation recipes.
Filled actions have separate normal, hover, and pressed backgrounds with a dedicated foreground.
Canvas selection keeps its own accent role.

## Values that stay local

| Category | Examples and reason |
| --- | --- |
| Edited document appearance | Inline text fonts, colors, alignment, spacing, and scale belong to the document. |
| Source defaults | Inserted-element sizes and shadow defaults change generated Slint source. They are document data. |
| Preview dimensions | `PreviewMetrics` defines the 390×720 device viewport, independently of editor styling. |
| Live state | `EditorWindow` dimensions, selection, scroll position, gestures, and resize snapshots are runtime state. |
| Unit conversion and safeguards | `1px`, zero lengths, clamping bounds, and angle conversions express algorithms. |
| Drawing and optical geometry | Icon strokes, glyph coordinates, brand proportions, and gradient tessellation define artwork. |
| Color-space mathematics | White/black saturation/value ramps and the hue spectrum define color selection, independently of the theme. |
| Interaction steps | Keyboard increments and angle snapping express input behavior, rather than visual spacing. |
| Platform geometry | Cursor hotspots and titlebar safe areas have platform-specific meaning. The authored titlebar inset has its own layout field. |

The shared LSP marker and color indicator accept appearance inputs.
The new editor passes its style explicitly; their defaults preserve the existing LSP picker appearance.
Do not make the legacy LSP styling depend on the new editor.

## Validation

Run the editor's Rust tests and Clippy gate, plus the UI suite with `SLINT_EDITOR_UI_TEST_BACKEND=headless-skia`.
The theme regression renders the same editor across light → dark → light and checks action contrast in all three interaction states.
The UI suite covers pointer mapping, picker placement, pane resizing, renaming, generated source, and undo behavior.
Inspect its rendered screenshots when changing appearance or geometry.
