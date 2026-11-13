---
name: slint
description: Write, debug, or review Slint UI code and its host-language integration, with or without the Slint plugin.
---

# Slint Development

Follow the project's Slint version, build setup, and existing design conventions.
For API details, use the matching [Slint documentation](https://slint.dev/docs) or `https://releases.slint.dev/<version>/docs`.
When `slint-docs` is available, search and fetch the relevant page; this skill doesn't require an MCP server.

## Check the Relevant Behavior

Use the project's compiler diagnostics and tests for code and integration changes.
Inspect a render before judging appearance; nonvisual edits don't require screenshots.
If the Slint plugin is available, follow its preview workflow.
Otherwise, use the installed `slint-viewer` options to check a file or render a screenshot.
Standalone previews don't execute host-language callbacks, models, or services; verify those in the running application.

## References

Read only the material relevant to the task.
Check version-specific details against the documentation.

| Topic | Reference |
|---|---|
| Components, bindings, and sizing | [Language and layout](reference/language-and-layout.md), [compile errors](reference/gotchas.md) |
| Project setup and application logic | [Setup](setup.md), [host integration](reference/interop.md) |
| Input, menus, and themes | [Events and overlays](reference/events-and-overlays.md), [icons and theming](reference/icons-and-theming.md) |
| Debugging and local tools | [Application debugging](reference/debugging-and-mcp.md), [tool installation](tools-install.md) |
| Visual review | [Polish](reference/polish.md) |
| Explicitly embedding Slint in a web page | [Web embedding](reference/web-embedding.md) |
| Licensing questions | [Licensing](reference/licensing.md) and its linked license terms |
