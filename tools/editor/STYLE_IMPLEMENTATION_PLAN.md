# Visual Editor Style Implementation Checklist

Branch: `nigel/editor-style`.
Base: freshly fetched `origin/master`, commit `e49dc84eb26417000422e85e1d6d9331635d3a01`.
Worktree: `/private/tmp/slint-editor-style`.

Implementation is in progress, starting with the shared style foundation.
Each numbered section is one proposed commit, in dependency order.
Complete the relevant checks before moving to the next commit.
Run UI tests with `SLINT_EDITOR_UI_TEST_BACKEND=headless-skia`, as requested.

## Resulting API

One `ui/style.slint` file owns an exported `Style` global with these grouped properties:

| Property | Responsibility |
| --- | --- |
| `Style.colors` | Surfaces, text, fields, actions, focus, selection, errors, and transparency checkerboards. |
| `Style.spacing` | Shared spacing steps and the source values for semantic padding/gap aliases. |
| `Style.typography` | Font family and caption, control, body, heading, section-heading, and hero text styles. |
| `Style.controls` | Control heights, padding, radii, borders, disabled opacity, and icon sizes. |
| `Style.layout` | Window defaults, pane geometry, splitters, bars, dialog dimensions, and image-editor rail metrics. |
| `Style.inspector` | Field columns, picker/menu geometry, gradient-stop columns, slider metrics, and dial brushes. |
| `Style.tree` | Navigation rows, indentation, disclosure hit targets, guides, and row states. |
| `Style.canvas` | Grid, outlines, visual handles, separate hit sizes, gradient overlays, and slice guides. |
| `Style.welcome` | Welcome surfaces, brand brush, shell geometry, actions, and recent-project rows. |
| `Style.shadows` | Popup, floating-panel, dialog, preview, and handle elevation recipes. |
| `Style.motion` | Fast feedback, normal feedback, and layout animation durations. |

Use `TextStyle { size: length, weight: int }` and `ShadowStyle { color: color, blur: length, offset-y: length }`.
Use named structs for groups where that improves readability, while preserving the `Style.colors.field` access pattern.
Use colors for text and color operations; use brushes for surfaces and decorative gradients.
Add fields only when a live consumer needs them.

## Base Update Since the Audit

The original audit used `9f97c5b892`; this plan incorporates changes on the newly fetched master:

- File renaming adds field surfaces, borders, error colors, and padding-dependent extension geometry.
- The new-file button adds a 28px control, a 16px icon, and disabled/hover styling.
- The new-file error popup adds dialog, error-surface, typography, and elevation roles.
- Inline canvas text editing reads fonts, colors, alignment, and spacing from the edited document.
  Preserve these document bindings instead of replacing them with editor typography.
- Existing `FontWeight` names remain preferable to introducing bare numeric weights at call sites.

## Commit 1: Introduce the Shared Style Foundation

Suggested subject: `Visual Editor: Introduce the shared Style API`

- [ ] Add `ui/style.slint`, the `Style` global, grouped types, and `TextStyle`/`ShadowStyle`.
- [ ] Read `Palette.color-scheme` centrally and provide reactive light/dark bindings.
- [ ] Move existing theme/token definitions into this ownership model without changing their values.
  Retain temporary variant fields where existing component palettes differ.
- [ ] Keep compatibility aliases in `StudioTheme`, `InspectorTokens`, and `EditorTreeTokens` during migration.
  Aliases must not introduce a second set of literal definitions.
- [ ] Separate live window dimensions from static metrics, updating picker consumers and Rust tests that set those dimensions.
- [ ] Move `InspectorComboOption` to a control-data location rather than retaining it in a styling file.
- [ ] Keep `style.slint` independent of `Api`, `Project`, `FillSession`, and component implementation files.
- [ ] Verify the editor compiles and both theme bindings update correctly in the real editor.
  Record baseline light/dark renders for the surfaces affected by later normalization.

## Commit 2: Migrate Shared Controls and Typography

Suggested subject: `Visual Editor: Apply shared styles to common controls`

- [ ] Migrate labels, search fields, sidebar toggles, palette rows, and divider components from `common.slint`.
- [ ] Extract repeated typography, control heights, borders, radii, focus treatments, and animation durations.
- [ ] Centralize splitter height and pane minimums while preserving keyboard steps and accessibility behavior.
- [ ] Keep distinct current values during extraction; normalization belongs to commits 7 and 8.
- [ ] Check pane resizing, search, palette dragging, focus, disabled states, and persisted pane sizes.
  Use the existing pane and palette tests for affected behavior.

## Commit 3: Migrate Inspector and Fill Picker Styling

Suggested subject: `Visual Editor: Centralize inspector and picker styling`

- [ ] Migrate fields, sections, copy buttons, menus, segments, sliders, dials, and swatches to `Style`.
- [ ] Centralize picker dimensions, stop-table columns, marker dimensions, checkerboard colors, and decorative dial brushes.
- [ ] Parameterize the shared LSP gradient marker and color indicator with appearance inputs.
  Pass new-editor styling explicitly and preserve default appearance for existing LSP consumers.
- [ ] Keep native menu placement, popup stacking, color-space ramps, and document effect defaults unchanged.
- [ ] Preserve the tested 24px stop-action widths and 48px Close widths.
- [ ] Run inspector, picker-layout, and gradient tests relevant to the changed controls.
  Check the legacy LSP picker still compiles and retains its appearance.
- [ ] Visually compare short and long stop lists, both themes, and stacked/side-by-side picker layouts.

## Commit 4: Migrate Navigation and File Operations

Suggested subject: `Visual Editor: Centralize navigation styling`

- [ ] Migrate file/outline rows, headings, guides, selection states, drag previews, and the new-file button.
- [ ] Route rename field background, normal/error border, and error text through the central style.
- [ ] Name navigation glyph and hit-target dimensions separately.
- [ ] Preserve rename extension width, focus, text selection, validation feedback, and expanded error-row height during extraction.
- [ ] Run navigation and outline tests, including create-file and rename success/failure cases.
  Check long filenames, deep indentation, keyboard focus, and drag/drop feedback visually.

## Commit 5: Migrate Shell, Welcome, and Dialog Styling

Suggested subject: `Visual Editor: Centralize shell and welcome styling`

- [ ] Migrate the top bar, Run button, update indicators, collapsed-sidebar panel, and startup wizard.
- [ ] Migrate the new-file error popup, including error surfaces, dialog elevation, footer, and Close action.
- [ ] Centralize welcome brand brushes and composition metrics without moving artwork coordinates into the style API.
- [ ] Preserve titlebar safe areas, window dragging/zoom, native menu behavior, and updater click boundaries.
- [ ] Run startup and navigation tests; inspect both themes with and without the startup wizard.
- [ ] Inspect long error messages, disabled actions, collapsed sidebars, and updater states without triggering an actual update.

## Commit 6: Migrate Canvas and Image Editor Styling

Suggested subject: `Visual Editor: Centralize canvas and image editor styling`

- [ ] Migrate the canvas grid, outlines, tooltip appearance, handles, and gradient overlays.
- [ ] Migrate image tabs, fields, syntax/copy controls, rail panels, slice guides, and preview shadows.
- [ ] Share grid rendering and appearance where practical; preserve rendering coverage until the layout normalization commit.
- [ ] Reuse common field appearance while retaining differences in content height and interaction behavior.
- [ ] Preserve visual handle sizes separately from hit areas and pointer-coordinate calculations.
- [ ] Preserve inline text editing's document-defined font, color, alignment, and scale bindings.
- [ ] Keep device preview sizes, inserted-element presets, cursor hotspots, and generated source defaults outside app styling.
- [ ] Run canvas, outline, inline-text, inspector-transform, gradient-canvas, and source-safety tests as affected.
  Visually inspect image preview, nine-slice guides, and resized previews in both themes.

## Commit 7: Unify Colors, Typography, Elevation, and Feedback

Suggested subject: `Visual Editor: Unify visual state styling`

- [ ] Merge equivalent primary/secondary text, field, and inactive tree-guide roles.
  Retain distinct surface, selection, drop-target, and active-guide semantics.
- [ ] Fix the welcome action's light-only pressed background through a theme-aware pressed role.
- [ ] Give filled actions a dedicated foreground/background pair with readable normal, hover, and pressed states.
  Apply this to Run and dialog Close without changing canvas selection blue.
- [ ] Normalize matching action typography, disabled opacity, and selected-control tints.
  Keep drag-source, drag-ghost, read-only, and disabled semantics separate.
- [ ] Consolidate equivalent floating shadows around 18px blur/8px offset, preserving distinct dialog, preview, and handle elevation.
- [ ] Normalize 140ms feedback to 150ms; retain 120ms fast feedback and 180ms welcome layout motion.
- [ ] Compare before/after renders across light/dark and focused, selected, hovered, pressed, disabled, and dragging states.
  Add targeted regression coverage only for meaningful behavior or appearance risks.

## Commit 8: Normalize Spacing and Derive Coupled Dimensions

Suggested subject: `Visual Editor: Normalize spacing and derive layout metrics`

- [ ] Apply 12px pane padding, 8px field/row padding, and 6px ordinary field gaps where the roles match.
  Retain 4px tightly grouped suffix/icon spacing and explicit optical exceptions.
- [ ] Normalize ordinary shell controls toward 32px, while keeping inspector 24px, palette 36px, and recent-project rows 44px.
  Keep the new-file header action's 28px target separately named unless rendered alignment supports changing it.
- [ ] Normalize welcome actions to 36px with an 8px gap, updating their combined height to 80px.
- [ ] Normalize compact/normal/panel container radii to 4/6/8px and derive pill/circle radii from size.
- [ ] Derive inspector content width from pane width, divider, and padding instead of retaining independent 207px literals.
- [ ] Update file rename extension geometry and image-panel inner widths together with their owning padding.
- [ ] Derive gradient marker insets, guide centering, knob centers, and slider offsets from the same metrics used for input mapping.
- [ ] Derive grid row/column counts from viewport dimensions and grid pitch.
- [ ] Recheck minimum-size windows, long labels, popup placement, hit targets, and normal/HiDPI rendering.
  Run affected pane, navigation, picker, canvas, and source-safety tests with geometry changes in the same commit.

## Commit 9: Remove Transitional Styling and Verify Coverage

Suggested subject: `Visual Editor: Remove obsolete styling definitions`

- [ ] Remove compatibility aliases and old token files once all consumers use `Style`.
- [ ] Remove temporary palette variants that have no remaining distinct role.
- [ ] Recheck references before deleting unused navigation/property components, the unused library row, and the unused segmented control.
- [ ] Confirm the inspector's image-mode branch is unreachable before removing it; retain shared components with other consumers.
- [ ] Remove the outline's residual debug border color and obsolete styling comments.
- [ ] Scan all editor UI files and imported shared widgets for remaining style literals and direct theme reads.
  Keep documented categories of legitimate local values rather than enforcing a zero-literals rule.
- [ ] Add a short style ownership guide explaining new-token criteria, derived geometry, and document-data boundaries.
- [ ] Run the final validation below and record results, skipped cases, screenshots, and any platform limitations.

## Validation and Completion

Run focused checks with each commit; add regression tests alongside the relevant change.
Avoid tests that merely assert a constant equals its own token definition.
Keep test fixtures and expected document output independent of editor styling.
Use existing source/preview synchronization helpers before pointer events and after edits.

For final Rust checks, use the editor's current CI commands from `.github/workflows/ci.yaml`:

```sh
cargo clippy --locked -p slint-editor --all-targets --all-features --features slint/mcp --timings -- -D warnings
cargo test --locked -p slint-editor --all-features --features slint/mcp --timings
```

Provision the new worktree's UI-test environment using `ui-tests/README.md`.
Build the executable required by the UI harness, then run the full suite with visible winit/Skia windows:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 SLINT_EMIT_DEBUG_INFO=1 cargo build --locked -p slint-editor --all-features --features slint/mcp
cd tools/editor/ui-tests
./run-tests.sh --visible
```

These feature flags match the existing harness; no MCP server interaction or backend changes are needed.
For manual checks, launch the real editor through `cargo run` as directed by `ui/AGENTS.md`.
Confirm the window is visible and compare renders, rather than treating successful launch as visual verification.

- [ ] One authoritative `Style` API controls the app's authored styling.
- [ ] Every active surface, including rename and error states added on master, is covered.
- [ ] Equivalent roles share values; purposeful differences remain named and understandable.
- [ ] Input geometry, generated documents, inline text appearance, and native behavior remain correct.
- [ ] Both themes and minimum-size/HiDPI layouts have been visually checked.
- [ ] Shared LSP widgets retain their default behavior and appearance.
- [ ] Application changes are confined to the new branch; publication is a separate step.
