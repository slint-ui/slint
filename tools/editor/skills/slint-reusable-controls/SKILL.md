---
name: slint-reusable-controls
description: Develop or review the reusable headless and Basic controls in the Slint repository, including callers choosing between styling properties and custom composition. Use for tools/editor/controls and its examples or tests; not for unrelated Slint UI work.
---

# Reusable Slint Controls

Read the repository's AGENTS.md and tools/editor/controls/README.md before changing these controls.
Inspect src/headless.slint and src/basic.slint under tools/editor/controls for the available components and their actual contracts.

## Choose the Component

Use a Basic control when its existing styling properties cover the customization, including colors, borders, fonts, dimensions, and padding.
For a button, start with `Button`, implemented in `tools/editor/controls/src/basic/button.slint` and exported by `src/basic.slint`.
Customize its instance through the existing properties.
Use `ButtonBase` only when those properties can't express the required content, structure, or visuals.
Apply the same rule to the other Basic controls and their corresponding headless *Base components.
For example, use ButtonBase for a custom icon-and-status layout and LineEditBase for leading or trailing adornments.
Supply the required visuals through the base's existing slots.
Reuse BasicTheme or a compatible Basic popup when helpful.

Keep one composition API on the headless components.
Don't add custom-* slots, use-default-* switches, or another visual replacement mechanism to Basic controls.
Basic controls fill the base slots and expose styling properties.
An ordinary slot assignment consumes that slot; it isn't an inherited default that callers can replace.
Forward a slot explicitly when a custom headless composition needs to accept caller-supplied visuals.

This design assumes the target compiler supports named and typed slots but doesn't support replaceable inherited default slot content.
Check the target branch's compiler and controls before applying that assumption to another branch or project.
If slot-default support becomes available and the user requests it, reconsider replaceable defaults through the original base slots.
Don't change compiler semantics as part of a controls-only customization.

## Keep Library Components Unchanged When Using Them

When using or customizing controls in an application, edit the caller's instances or create application-owned components that compose or inherit the existing controls.
Don't edit the reusable Basic components, headless bases, or shared theme in `tools/editor/controls/src/` to satisfy a caller's design.
Keep their existing public API unchanged, including properties, callbacks, functions, slots, interfaces, and exports.
Don't add library properties or slots, change defaults, or alter interaction behavior to make a customization possible.
Put custom visuals and any application-specific properties in the caller's own component.
If the requirement can't be met through the existing API, explain the limitation instead of silently extending the library.

Change library implementations or APIs only when the user explicitly requests work on the reusable controls themselves.
A request to customize a button or other caller doesn't authorize such changes.

## Preserve the Behavior

Keep interaction, focus, callbacks, interfaces, and accessibility in the headless implementation.
Bind supplied visuals to the base's exposed state, including enabled, hovered, pressed, checked, and has-focus where applicable.
Keep text or accessible-label meaningful when visible content differs from the label.
Let custom content determine layout together with padding and explicit size constraints.
Provide required typed slots, such as ComboBoxBase.popup and SliderBase.surface, with components implementing their declared interfaces.
Use LinearSliderSurface to retain linear pointer mapping when customizing a slider's track and handle.
Keep TextInput ownership in LineEditBase when adding visuals or adornments.

For explicitly requested library changes, update examples and the controls README when the customization contract changes.
Preserve the intended appearance while migrating a caller from Basic to a base component.

## Validate the Change

Enable SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 for these experimental slots.
Use the target branch's compiler to validate syntax and contracts; an installed viewer may use a different compiler.
Run the relevant reusable control cases in the separate tests Cargo workspace.
For changes across these controls, run the filtered interpreter and Rust component-slot suites:

```powershell
$env:SLINT_ENABLE_EXPERIMENTAL_FEATURES = "1"
$env:SLINT_TEST_FILTER = "reusable_"
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter
cargo test --manifest-path tests/Cargo.toml -p test-driver-rust --test component_slots
```

Use the repository's required toolchain when necessary.
For visual changes, render and inspect the affected gallery pages in tools/editor/controls/examples/controls.slint.
Compare against the previous appearance with the same renderer when checking visual equivalence.
Verify interaction and accessibility with behavior tests; a matching screenshot alone doesn't establish those contracts.
