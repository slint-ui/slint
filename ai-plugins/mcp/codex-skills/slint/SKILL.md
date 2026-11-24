---
name: slint
description: Write, edit, debug, or review Slint UIs and host-language integration. Use for Slint docs, development guidance, inline previews, and preview annotations.
---

# Slint

## Find the Slint Project

Look beside the target `.slint` file and up through the workspace for the applicable `slint-project.json`.
Resolve `main`, include paths, and library paths relative to the directory containing the project file.
Use `main` for an app preview when the project file selects the entry point.

For Rust, check the owning crate's `Cargo.toml` and any inherited workspace dependencies.
If the crate uses `slint_build`, read its build script for the entry point and compiler settings.
Use `Cargo.lock` for the resolved Slint version.
The project file's `edition` controls language behavior.

## Look Up Slint APIs

Find the project's Slint version in its build configuration or lockfile.
Call `slint-docs` `search` for the element, property, widget, or API you need.
Pass `version` when the project uses an older release.
Call `fetch` with the result's URL exactly as returned to read the page for that release.

If the documentation tools are unavailable, open the matching release at
`https://releases.slint.dev/<version>/docs` or use the [current Slint docs](https://docs.slint.dev/).

## Check and Preview

Set `prepare_preview: true` when the session supports MCP Apps and you want to show an interactive preview.

1. Call `check_slint` with the saved file's absolute `path`.
   If a `slint-project.json` applies, pass its containing directory as the absolute `projectRoot`.
   Use `source` for a self-contained snippet.
2. Inspect the diagnostics and PNG.
   Check that layout children align as expected.
   Fix issues and run the check again.
3. After a successful check, call `publish_preview` with the returned `previewId` if you prepared a preview.
   Run another check if the preparation has expired or was already used.

Use the PNG from `check_slint` for the visual review and to share the render.
In Codex code mode, forward the tool's image and text blocks so the screenshot and diagnostics are visible.
When the user annotates an inline preview, use the attached screenshot and any source location to identify the element.

Use the running app or its tests to verify host-language callbacks, models, and application state.

## `.slint` in 30 Seconds

Declarative and reactive: a property binding re-evaluates automatically when
anything it reads changes.

```slint
import { Button, VerticalBox } from "std-widgets.slint";

component Counter inherits Rectangle {     // root element decides fill behavior
    in property <string> label;            // parent/host writes
    out property <int> count;              // component writes
    callback changed(int);                 // notify the outside world

    VerticalBox {
        Text { text: "\{root.label}: \{root.count}"; }   // interpolation
        Button { text: "+"; clicked => { root.count += 1; root.changed(root.count); } }
    }
}
```

Property directions: `in` / `out` / `in-out` / `private`. Two-way bind: `a <=> b`.
Control flow: `if cond : E {}`, `for it[i] in model : E {}`. Shared state & host
interop: `export global Foo { ... }`. One-time code: `init => { ... }`.

## Reference Files

Read the matching reference before working in that area.

| File | Read when… |
|---|---|
| [setup.md](guidance/setup.md) | Starting a project / wiring the build (Rust/C++/Node/Python). |
| [language-and-layout.md](guidance/reference/language-and-layout.md) | Writing components; an element won't size/fill as expected. |
| [gotchas.md](guidance/reference/gotchas.md) | A file won't compile, or colors/units/math/enums behave oddly. |
| [events-and-overlays.md](guidance/reference/events-and-overlays.md) | Clicks/keys/modifiers, or popovers/menus/context menus. |
| [icons-and-theming.md](guidance/reference/icons-and-theming.md) | Icons, or light/dark theming. |
| [interop.md](guidance/reference/interop.md) | Connecting the UI to host-language logic (models, callbacks, globals). |
| [polish.md](guidance/reference/polish.md) | The UI works but looks rough; reviewing a rendered screenshot. |
| [debugging-and-mcp.md](guidance/reference/debugging-and-mcp.md) | Debugging a running app or taking a screenshot with `slint-viewer`. |
| [licensing.md](guidance/reference/licensing.md) | The user asks about the license, or builds a closed-source app. |
| [web-embedding.md](guidance/reference/web-embedding.md) | Showing a live `.slint` preview in a web page, HTML report, or docs. |
| [tools-install.md](guidance/tools-install.md) | Installing Slint development tools. |
