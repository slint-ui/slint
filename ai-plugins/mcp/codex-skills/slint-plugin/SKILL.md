---
name: slint-plugin
description: Write, edit, debug, or review Slint UIs and host-language integration in Codex. Use for Slint docs, development guidance, inline previews, and preview annotations.
---

# Slint Plugin for Codex

## Find the Slint Project

Look beside the target `.slint` file and up through the workspace for the applicable `slint-project.json`.
Resolve `main`, include paths, and library paths relative to the directory containing the project file.
Use `main` for an app preview when the project file selects the entry point.

For Rust, check the owning crate's `Cargo.toml` and any inherited workspace dependencies.
If the crate uses `slint_build`, read its build script for the entry point and compiler settings.
Use `Cargo.lock` for the resolved Slint version. The project file's `edition` controls language behavior.

## Look Up Slint APIs

Find the project's Slint version in its build configuration or lockfile.
Call `slint-docs` `search` for the element, property, widget, or API you need.
Pass `version` when the project uses an older release.
Call `fetch` with the result's URL exactly as returned to read the page for that release.

If the documentation tools are unavailable, open the matching release at
`https://releases.slint.dev/<version>/docs` or use the [current Slint docs](https://docs.slint.dev/).

## Preview in Codex

Use the inline preview in Codex sessions that can display MCP Apps.

1. Save the `.slint` file and pass its absolute `path` to `preview_slint` so imports and assets resolve.
   When a `slint-project.json` applies, pass its containing directory as the absolute `projectRoot`.
   Use `source` for a self-contained snippet that imports only `std-widgets.slint`.
2. Keep the returned preview ID and pass it as `previewId` to `get_preview_screenshot`.
   Inspect the image and any diagnostics before confirming the UI rendered.
   Check the preview screenshot to verify that children of each layout line up along the intended edges or center lines.
3. If the screenshot is pending, call `get_preview_screenshot` again with the same ID.
   If startup is unconfirmed, ask the user to open the chat, then retry that ID.
   Stop automatic retries when the tool reports a timeout.
4. Open a fresh preview after saving changes.
   When the user annotates a preview, use the attached screenshot and any source location to find the element they mean.

To show a screenshot in Codex code mode, forward the returned image block directly.
Forward text blocks as text so diagnostics and pending states remain visible.

```javascript
const result = await tools.mcp__slint_preview__get_preview_screenshot({ previewId });
for (const block of result.content ?? []) {
    if (block.type === "image") image(block);
    if (block.type === "text") text(block.text);
}
```

For a terminal-only session, use `slint-viewer` to check and render the file locally.
Pass `--project-root <directory>` to apply project settings to a specific `.slint` file.
Use the running app or its tests to verify host-language callbacks, models, and application state.
See [local rendering and runtime debugging](../../guidance/reference/debugging-and-mcp.md).

## Find Slint Development Guidance

For component work, read [Language and Layout](../../guidance/reference/language-and-layout.md) before editing.
For project setup and build wiring, read [Project Setup](../../guidance/setup.md).
Use the [development references](../../guidance/reference/) for common errors, host integration, and visual polish.
