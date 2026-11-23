---
name: slint
description: Write, edit, debug, or review Slint UIs and host-language integration in Claude Code. Use for Slint docs, development guidance, compiler diagnostics, and first-frame screenshots.
---

# Slint Skill for Claude Code

## Find the Slint Project

Look beside the target `.slint` file and up through the workspace for the applicable `slint-project.json`.
Resolve `main`, include paths, and library paths relative to the directory containing the project file.
Check `main` when the project file selects the entry point.

For Rust, check the owning crate's `Cargo.toml` and any inherited workspace dependencies.
If the crate uses `slint_build`, read its build script for the entry point and compiler settings.
Use the requested UI version, defaulting to the version resolved in `Cargo.lock`.
The project file's `edition` controls language behavior.

## Look Up Slint APIs

Call `slint-docs` `search` for the element, property, widget, or API you need.
Pass `version` for the target release.
Pass `site: "testing"` to search the Slint UI Testing docs.
Call `fetch` with the result's URL exactly as returned to read the page for that release.

If the documentation tools are unavailable, open the matching release at
`https://releases.slint.dev/<version>/docs` or use the [current Slint docs](https://docs.slint.dev/).

Use the `slint-lsp` language server for hover, go-to-definition, and source diagnostics when it is installed.

## Check Slint and Inspect the First Frame

1. Call `slint-preview`'s `check_slint` with the saved file's absolute `path`.
   If a `slint-project.json` applies, pass its containing directory as the absolute `projectRoot`.
   For a self-contained snippet, pass `source` instead of `path`.
   Pass `style` if the project uses a specific standard widget style.
2. Inspect the compiler diagnostics and the first-frame PNG.
   Check that layout children align as expected.
   Fix issues and run the check again.

Use the returned PNG when sharing a screenshot.
For host integration changes, verify callbacks and models in the running app.

## `.slint` in 30 Seconds

Slint is declarative and reactive: a property binding re-evaluates automatically when
anything it reads changes.
 
Read all files in ../../guidance/examples/music-player/ for a short example project.
 
Property directions: `in` / `out` / `in-out` / `private`. Two-way bind: `a <=> b`.
Control flow: `if cond : E {}`, `for it[i] in model : E {}`. Shared state & host
interop: `export global Foo { ... }`. One-time code: `init => { ... }`.

## Reference Files

Read the matching reference before working in that area.

| File | Read when… |
|---|---|
| [setup.md](../../guidance/setup.md) | Starting a project / wiring the build (Rust/C++/Node/Python). |
| [language-and-layout.md](../../guidance/reference/language-and-layout.md) | Writing components; an element won't size/fill as expected. |
| [project-structure](../../guidance/reference/project-structure.md) | Best Practices & Target Project Structure for a multi-page UI | 
| [gotchas.md](../../guidance/reference/gotchas.md) | A file won't compile, or colors/units/math/enums behave oddly. |
| [events-and-overlays.md](../../guidance/reference/events-and-overlays.md) | Clicks/keys/modifiers, or popovers/menus/context menus. |
| [icons-and-theming.md](../../guidance/reference/icons-and-theming.md) | Icons, or light/dark theming. |
| [interop.md](../../guidance/reference/interop.md) | Connecting the UI to host-language logic (models, callbacks, globals). |
| [polish.md](../../guidance/reference/polish.md) | The UI works but looks rough; reviewing a rendered screenshot. |
| [debugging-and-mcp.md](../../guidance/reference/debugging-and-mcp.md#mcp-server-for-ai-assisted-debugging) | Debugging or interacting with the running app through its embedded MCP server. |
| [licensing.md](../../guidance/reference/licensing.md) | The user asks about the license, or builds a closed-source app. |
| [web-embedding.md](../../guidance/reference/web-embedding.md) | Showing a live `.slint` preview in a web page, HTML report, or docs. |
| [tools-install.md](../../guidance/tools-install.md) | Installing `slint-lsp`. |
