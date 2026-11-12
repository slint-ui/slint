---
name: slint-plugin
description: Create, edit, and inspect Slint UIs using this plugin's documentation and preview tools.
---

# Use the Slint Plugin

## Documentation

Use the `slint-docs` MCP server: call `search`, then `fetch` with a result's URL unchanged.
Both tools default to the current Slint release.
For an older project release, pass its `version` to `search`; the returned URL makes `fetch` use the same release.

## Inline Preview

Use this workflow in Codex hosts that can display and operate inline MCP Apps.
The presence of preview tools alone doesn't establish that the host supports their view.

1. Save changes and call `preview_slint` with the file's absolute `path` so imports and assets resolve.
   Pass `projectRoot` when applying a `slint-project.json`; use `source` for self-contained snippets importing only `std-widgets.slint`.
2. Keep the returned preview ID and call `get_preview_screenshot` with that `previewId`.
   A preview request acknowledges the call; inspect the screenshot and diagnostics before confirming a successful render.
   Heed Wasm preview warnings: remove code reported as unnecessary before using the code further.
3. On `pending`, request another screenshot for the same ID.
   If startup is unconfirmed, ask the user to open its chat and retry that ID after they return.
   Stop automatic retries on timeout; don't create replacement cards just because startup is delayed.
4. After source changes, preview the saved revision again; existing cards don't watch files.
   Interpret user annotations using their attached screenshot and source information.

## Show the Screenshot

The screenshot tool returns an image content block containing the PNG.
Forward that block through the host's image output unchanged.
Don't turn it into a Markdown image link, construct a `data:` URL, or replace its data with a placeholder.
Printing the tool result as text doesn't display its image.

In Codex code mode, forward image blocks with `image()` and text blocks with `text()`:

```javascript
const result = await tools.mcp__slint_preview__get_preview_screenshot({ previewId });
for (const block of result.content ?? []) {
    if (block.type === "image") image(block);
    else if (block.type === "text") text(block.text);
}
```

If the tool returns a pending status or an error without an image, report that result instead of showing an empty image.

## Local Tooling

Use local tools in terminal-only Codex CLI, Claude, or another host without a working inline preview.
Check `slint-viewer --version` and `--help`, then use its supported check and screenshot commands:

```sh
slint-viewer --check ui/main.slint
slint-viewer --screenshot /absolute/path/preview.png ui/main.slint
```

Inspect the local PNG with the host's image tools and share its absolute path when appropriate.
Use `--auto-reload` when a visible local window helps the task.
Report unavailable tools or image inspection rather than claiming visual success.

Standalone previews don't execute host-language application logic; verify that through the running application or its tests.
