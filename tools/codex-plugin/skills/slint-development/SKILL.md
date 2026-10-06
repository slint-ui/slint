---
name: slint-development
description: Develop, edit, validate, and preview Slint applications and components. Use when the user asks to build or change a Slint UI or diagnose Slint source errors.
---

# Slint Development

Work directly in the user's Slint project.
Read its instructions, dependencies, and existing components before editing.
For a standalone component preview, use the bundled starter rather than inspecting unrelated project dependencies.
Use `.slint` as the canonical design source.
Preserve unrelated edits and use `apply_patch` for source changes.
Do not substitute an HTML implementation of the design.
Ask before adding UI elements, decoration, or behavior beyond the user's request.

Use the plugin's `validate_slint` tool to check a saved source file.
It runs the language server built from this monorepo checkout and returns diagnostics, a source hash, and a revision.
Success is `structuredContent.status === "valid"`.
The other statuses are `"error"` for Slint errors and `"failure"` for validator or setup errors.
Do not check for `"ok"`.
Do not search for another validator when this tool is available.
The preview and validator use the same Slint source revision recorded in `runtime/runtime.json`.
If runtime files are missing, run `node tools/codex-plugin/scripts/build-runtime.mjs` from the monorepo before installing the plugin.
Build the runtime after updating the branch from master; do not substitute a released LSP or Wasm interpreter.
Check the project's Slint dependencies before relying on master for another version.

For a requested Button design, use the starter included in `render_slint`'s description.
Change only the requested properties; preserve implicit centering, geometry, and state-color defaults.
Do not add `x`, `y`, hover, or pressed overrides unless requested.
Use `show_slint_button` when the user wants the unchanged example.
Import `Button` from `slint-button.slint`.
Its API includes `label`, `label-color`, `background-color`, `hover-color`, `pressed-color`, `disabled-color`, `enabled`, and `clicked()`.
Keep the Window transparent unless the user requests a background.
Set root dimensions explicitly and pass the same logical dimensions to `render_slint`.
The preview resolves relative `.slint` imports, PNG/JPEG/SVG/WebP images, and TrueType/OpenType fonts.
Pass `projectRoot` when dependencies extend beyond the source directory.
Keep dependencies inside that root; network imports, include-path aliases, and absolute dependency paths are outside this prototype's supported scope.

Use one code-mode execution to save with `apply_patch`, validate, and render.
Await validation and check its status before rendering; report errors and stop that execution if validation fails.
After defining the patch and its exact source, use this sequence:

```js
await tools.apply_patch(patch);
const checked = await tools.mcp__slint__validate_slint({ path, revision });
if (checked.isError || checked.structuredContent.status !== "valid") {
  text(checked);
} else {
  text(await tools.mcp__slint__render_slint({ path, revision, width, height, validatedSourceHash: checked.structuredContent.sourceHash }));
}
```

Render the exact bytes that passed validation, with the same revision and matching Window dimensions.
A tool response confirms submission.
Matching `slintPreview.state === "ready"` in model context acknowledges compilation, showing the instance, and a paint opportunity.
Use matching source path, revision, and hash; do not treat stale, draft, or missing acknowledgements as success.
`slintPreview.state === "error"` carries frontend diagnostics; report or repair that error rather than claiming the preview succeeded.
This acknowledgement does not independently verify pixels; the user can still inspect the visible result.
The CLI can use the tools and skill but does not provide an inline visualization surface.

For a follow-up edit, read `slintPreview.sourcePath`, `projectRoot`, `revision`, and `sourceHash` from model context.
Read that saved file and check its hash before patching.
Preserve its path and increment the revision for each edit; reconcile external changes before patching stale source.
Validate the entry file after changes to an imported component, then render that entry with its original project root.

When the user submits "Apply my Slint changes", read `slintEdit` from the accompanying model context.
Check its base source hash before changing the working file.
Use `slintEdit.sourcePath` and `projectRoot` to locate that file; never create a replacement file for an existing edit.
Apply only the requested properties, preserve other source, validate, and render the next revision.
If the edit context is missing or stale, reconcile it with the user rather than guessing.
Component edits do not require rebuilding or publishing the plugin.
