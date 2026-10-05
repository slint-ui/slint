---
name: slint-development
description: Develop, edit, validate, and preview Slint applications and components. Use when the user asks to build or change a Slint UI or diagnose Slint source errors.
---

# Slint Development

Work directly in the user's Slint project.
Read its instructions, dependencies, and existing components before editing.
Use `.slint` as the canonical design source.
Preserve unrelated edits and use `apply_patch` for source changes.
Do not substitute an HTML implementation of the design.
Ask before adding UI elements, decoration, or behavior beyond the user's request.

Use the plugin's `validate_slint` tool to check a saved source file.
It runs the language server built from this monorepo checkout and returns diagnostics, a source hash, and a revision.
Do not search for another validator when this tool is available.
The preview and validator use the same Slint source revision recorded in `runtime/runtime.json`.
If runtime files are missing, run `node tools/codex-plugin/scripts/build-runtime.mjs` from the monorepo before installing the plugin.
Build the runtime after updating the branch from master; do not substitute a released LSP or Wasm interpreter.
Check the project's Slint dependencies before relying on master for another version.

For the supplied reusable Button, use `show_slint_button` or start from `examples/button.slint` in this plugin.
Import `Button` from `slint-button.slint`.
Its API includes `label`, `label-color`, `background-color`, `hover-color`, `pressed-color`, `disabled-color`, `enabled`, and `clicked()`.
Keep the Window transparent unless the user requests a background.
Set root dimensions explicitly and pass the same logical dimensions to `render_slint`.
The preview resolves the bundled Button import; other project imports require the project's normal preview tooling.

Read, edit, validate, and render in one execution when possible.
Render the exact bytes that passed validation, with the same revision and matching Window dimensions.
A tool response confirms submission, not visible rendering.
Ask the user to inspect the preview when visual confirmation is needed.
The CLI can use the tools and skill but does not provide an inline visualization surface.

When the user submits "Apply my Slint changes", read `slintEdit` from the accompanying model context.
Check its base source hash before changing the working file.
Apply only the requested properties, preserve other source, validate, and render the next revision.
If the edit context is missing or stale, reconcile it with the user rather than guessing.
Component edits do not require rebuilding or publishing the plugin.
