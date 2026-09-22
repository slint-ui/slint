<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Gallery Deletion Audit

The refactor removes the scene model, property emulation, geometry editing, hit testing, snapshots, undo, redo, and application actions.
It removes the composition, full inspector, canvas, file tree, image editor, startup, and toolbar/status pages.
The canvas child hook, popup extraction, cursor extraction, and image-helper extraction are also removed from the branch.

Gallery-specific Rust falls from 1,358 to 330 lines: 202 for fixtures and callbacks, 52 for the catalog, and 76 for launch options.
Six pages remain, with 17 named scenarios.
The shared brush, gradient, palette, and recent-fill helpers are production code used by both binaries.
Keep these shared implementations; deleting them would require duplication or reimplementing component behavior.

The second pass found no remaining gallery document or history engine.
It removed the inert foundations reset button and corrected the basic-control examples' unsupported icons and overlapping segments.
The table covers every remaining sample and gallery control.
Removal suggestions below are optional further cuts, not unfinished refactor work.

| Area | Everything shown | Recommendation |
| --- | --- | --- |
| Foundations | Background, surface, subtle, foreground, muted, border, accent, focus, selection, and danger swatches | Keep for checking light and dark themes. |
| Foundations | Heading, body, and muted typography | Keep; three representative styles are enough. |
| Foundations | Small, medium, and large corner radii | Remove if the gallery should contain only interactive components. |
| Foundations | Seven spacing tokens | Remove; these static measurements add little to interaction testing. |
| Foundations | Floating picker shadow sample | Remove; the actual picker already shows this shadow. |
| Foundations | Rectangle, Text, Image, and TouchArea icon rows | Remove; all four appear in the element palette. |
| Basic controls | Text input | Keep for typing and focus inspection. |
| Basic controls | Sidebar toggle button | Remove; it only reports a click and adds little without a sidebar. |
| Basic controls | Visibility and fill icon buttons | Keep one representative icon button; remove the second if its appearance needs no separate inspection. |
| Basic controls | One, Two, and Three text segments | Keep to inspect selection, hover, and keyboard focus. |
| Basic controls | Standalone Rectangle palette row | Remove; the palette covers this component. |
| Basic controls | Horizontal resize divider | Keep for its pointer and keyboard behavior. |
| Basic controls | Last-click label | Keep only while the sample buttons need visible feedback. |
| Inspector controls | Numeric field, editable text field, and expression field | Keep; they have different editing presentations. Values remain local and expressions are not evaluated. |
| Inspector controls | Contain, cover, and fill combo box | Keep as one selectable menu example. |
| Inspector controls | One standalone InspectorSlider and its value label | Keep; this is master's shared slider, not another implementation. |
| Inspector controls | Rotation knob | Keep for circular dragging and keyboard interaction. |
| Inspector controls | Shadow-angle dial | Remove; it wraps the same rotation knob with an angle offset. |
| Inspector controls | Corner editor: all/separate modes, shared slider, and four fields | Keep; the compound interaction is distinct from the standalone slider. |
| Element palette | Search, expandable Visual and Input & interaction groups, and four primitive cards | Keep; explicitly requested. |
| Element palette | Drop target and last-drop label | Keep; minimal feedback without creating elements. |
| Element palette | Default, unavailable, and dragging-disabled scenarios | Keep; they expose distinct component states. |
| Fill picker | Sample swatch, open/close controls, solid/gradient tabs, color plane, hue, alpha, and color fields | Keep; explicitly requested. |
| Fill picker | Linear/radial/conic settings, stop ramp, stop rows, add/remove controls, and stop-color panel | Keep inside the production picker; avoid separate duplicate pages. |
| Fill picker | Recent fills | Keep inside the picker; no separate gallery history model is needed. |
| Fill picker | Default, transparent, linear, radial, conic, and unsupported scenarios | Keep gradient and unsupported states; transparent could be removed because opacity is directly editable. |
| Outline | Main with card, title, and artwork children; selection, expansion, drag preview, and drop feedback | Keep; explicitly requested. Drops report the destination and do not restructure a document. |
| Outline | Default, collapsed, long names, empty, and unavailable scenarios | Keep long names and unavailable states; collapsed can be reached by clicking Main. |
| Gallery shell | Six navigation buttons and page search | Keep navigation; remove search while there are only six pages. |
| Gallery shell | Page titles, descriptions, and section labels | Keep titles; remove repeated section labels where the page title already identifies the sample. |
| Gallery shell | System, light, and dark theme selector | Keep for visual inspection. |
| Gallery shell | Scenario selector, reset button on interactive pages, and scroll containers | Keep; they support disposable sample data and smaller windows. |

The next useful cut is the duplicate icon rows, standalone palette row, shadow sample, and shadow-angle dial.
That would remove redundant demonstrations without reducing the requested palette, picker, or outline coverage.
