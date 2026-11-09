# Visual Editor Agent Notes

Do not add `ChangeLog:` commit trailers unless the user explicitly requests them.

Read [Test Best Practices](TESTING.md) before changing or running Visual Editor tests.
Follow the additional UI guidance in [ui/AGENTS.md](ui/AGENTS.md) when editing the editor shell.

## Property Inspector Architecture and Workflow

The Visual Editor is primarily designed to edit the value of properties as they are **in the Slint source code**.
The runtime value should usually not be used when editing properties or showing property values.

In order to provide instant feedback without updating the Slint source and waiting for a reload, the editor should use this workflow when editing properties:

1. Apply immediate changes using the Slint debug hooks
    - Use `Properties.inspector-preview(name, value)` with a `PropertyEditValue` to stage and preview each property change.
    - This shows the value in the runtime immediately as-if the value had been set in the source
    - Should be used e.g. while dragging a slider
2. Commit the value to source once the user interaction ends
    - Use `Properties.inspector-commit()` when the interaction ends to commit all staged changes together.
    - The backend compares them with source, so callers do not need their own equality checks.
    - After the source edit arrives, the preview will reload, the debug hook will clear and the actual source value will be set.
    - Use `Properties.inspector-cancel()` to discard the staged changes and restore the previous overrides.

When displaying a property, the editor should therefore show:

1. The debug hook value, if one is set
   * This indicates a pending edit
2. The source value

This is implemented in `Properties.editor-property-value-data()`.
Always report if you need to use another function to display a property value, flag this to the user explicitly and ask how to proceed!

**Without explicit approval, DO NOT:**

- Fall back to the runtime value
- Try to track the pending edit in the Slint UI
- Track the WorkspaceEdit through the source-edit cycle.
    - After the source edit completes, the debug hooks will clear and the property inspector will show the source values again.
    - The editor must not anticipate any particular source edit. It must always react to the source as it is on disk. The source is authoritative.


### Property APIs

- Use `Properties.editor-property-value-data(name)` for property editor displays: active debug-hook override first, then source value.
- Use `Properties.hooked-property-value-data(hook-name, fallback)` when hook and source names differ or a custom fallback is needed.
  It returns the active debug-hook override, otherwise the supplied fallback.
- Use `Properties.source-property-value-data(name, fallback)` for authored expressions, units, binding presence, and comparisons that decide whether to submit edits.
  These must remain independent of debug-hook overrides.
- Use `Properties.runtime-property-value-data(name, fallback)` only when the actual running value is required, such as matching rendered text.
  It includes active debug hooks; wrapping it in another hook lookup is redundant.
  Keep geometry editors on the selection geometry APIs.
