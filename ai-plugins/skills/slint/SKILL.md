---
name: slint
description: Use when creating, editing, debugging, or previewing Slint UIs. Guides use of the Slint plugin's documentation and preview tools.
---

# Use the Slint Plugin

Use this plugin's tools for Slint documentation and previews.
Choose the preview workflow by the current host's capabilities, not merely the presence of a tool named `preview_slint`.

## Documentation

Use `slint-docs` to search for API details, then fetch the relevant page.
Check the project's Slint version when compatibility matters.
If the indexed documentation targets another version, use `https://releases.slint.dev/<version>/docs`.

## Inline Preview: Codex With MCP App Support

Use the plugin's inline preview as the normal preview workflow in these hosts.
Use local tooling instead only when requested or when inline preview is unavailable.

1. Call `preview_slint` with the saved file's absolute `path` so imports and assets resolve.
   Pass `projectRoot` when applying a `slint-project.json`; use `source` for self-contained snippets importing only `std-widgets.slint`.
2. Keep the returned preview ID and call `get_preview_screenshot` with that `previewId`.
   Inspect the returned image directly through the host's image output before confirming the render succeeded.
3. On `pending`, request another screenshot for the same ID.
   If startup is unconfirmed, ask the user to open its chat, then retry that ID after they return.
   Stop automatic retries on timeout; don't create replacement cards just because startup is delayed.
4. After source changes, preview the saved revision again; existing cards don't watch files.
   Interpret user annotations using their attached screenshot and source information.

## Local Tooling: Codex CLI and Claude

Codex CLI and other hosts without inline MCP App support must use local tooling, even if preview tools are listed.
The Claude plugin has no inline preview server; it provides documentation and declares `slint-lsp` for supported hosts.
Check local tool availability and supported options with `--version` and `--help` in the environment where the assistant runs.

```sh
slint-viewer --check ui/main.slint
slint-viewer --screenshot /absolute/path/preview.png ui/main.slint
```

Use language-server diagnostics when available and inspect the local screenshot with the host's image tools.
Use `slint-viewer --auto-reload` when a visible local window is requested or helps the task.
Report any inability to render or inspect the UI.

## Verification and Optional References

Inspect a render before confirming a visual change; documentation questions and nonvisual edits don't require screenshots.
Standalone previews don't run the application's host-language logic; verify that through the running application and its tests or application MCP server.
For additional language help, read [layout](reference/language-and-layout.md), [compile errors](reference/gotchas.md), or [host integration](reference/interop.md) as needed.
These references supplement the host workflow above; they don't select a different preview tool.
