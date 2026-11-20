---
name: slint
description: Use when writing, editing, debugging, or reviewing `.slint` UI code, or wiring it to Rust/C++/JS/Python — the Slint language, layout, compile-time gotchas, interop, and the MCP server for runtime checks.
---

# Slint Development Skill

For building, debugging, or reviewing apps that use [Slint](https://slint.dev),
a declarative GUI toolkit for desktop, embedded, mobile, and web.

For application UI design or refactoring, start with [project structure and state ownership](guidance/reference/project-structure.md).

## Workflow

1. Find the project's Slint version first (`Cargo.toml`/`Cargo.lock`,
   `package.json`, `pyproject.toml`, or the CMake `find_package`/`FetchContent`
   line). This skill targets Slint **≥ 1.17** and its prose assumes the latest
   release; anything that needs a specific version is flagged inline as
   `(1.17+)`, `(1.18+)`, … When the project pins an *older* version, or for
   exact element/property/widget signatures, trust that version's docs over
   this file rather than guessing.
2. After editing, check the compiler diagnostics.
   An IDE with the Slint extension reports them after each edit.
   With Slint 1.17 or later, run `slint-viewer --check ui/main.slint` to check a file directly.
   See [debugging-and-mcp.md](guidance/reference/debugging-and-mcp.md) for details.
3. Run `slint-viewer --screenshot preview.png ui/main.slint` and inspect the render.
   Review against [polish.md](guidance/reference/polish.md).
   Use the running app to verify interactions.
4. Share the render when the host supports it: inline the screenshot in chat
   apps, or print its absolute path and summarize the visual checks in CLI-only
   environments.
5. Offer to run `slint-viewer --auto-reload ui/main.slint` so the user watches
   changes live while you edit.

## Standalone Component Previews

For a new standalone component preview, start with a transparent Window:

```slint
export component Preview inherits Window {
    width: 320px;
    height: 160px;
    background: transparent;
}
```

Place the requested component directly inside the Window with explicit dimensions.
Leave its position unset so Slint centers it, and size the Window to leave modest space around it.
Add a background only when the user requests one.
Preserve the window, background, and layout of an existing application.

Most "won't compile" / "won't fill" / "padding ignored" questions are answered
in [gotchas.md](guidance/reference/gotchas.md) and
[language-and-layout.md](guidance/reference/language-and-layout.md).

## Reference Files

Skim the matching file *before* building in that area, not only when stuck.

| File | Read when… |
|---|---|
| [setup.md](guidance/setup.md) | Starting a project / wiring the build (Rust/C++/Node/Python). |
| [language-and-layout.md](guidance/reference/language-and-layout.md) | Writing components; an element won't size/fill as expected. |
| [gotchas.md](guidance/reference/gotchas.md) | A file won't compile, or colors/units/math/enums behave oddly. |
| [events-and-overlays.md](guidance/reference/events-and-overlays.md) | Clicks/keys/modifiers, or popovers/menus/context menus. |
| [icons-and-theming.md](guidance/reference/icons-and-theming.md) | Icons, or light/dark theming. |
| [interop.md](guidance/reference/interop.md) | Connecting the UI to host-language logic (models, callbacks, globals). |
| [polish.md](guidance/reference/polish.md) | The UI works but looks rough; reviewing a rendered screenshot. |
| [debugging-and-mcp.md](guidance/reference/debugging-and-mcp.md) | Runtime debugging, headless/CI rendering, screenshots, the MCP server. |
| [licensing.md](guidance/reference/licensing.md) | The user asks about the license, or builds a closed-source app. |
| [web-embedding.md](guidance/reference/web-embedding.md) | Showing a live `.slint` preview in a web page, HTML report, or docs. |
| [tools-install.md](guidance/tools-install.md) | Installing `slint-lsp` (language server) or `slint-viewer` (preview / screenshots). |

## `.slint` in 30 seconds

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

## Documentation

The docs are the authority on element, property, and widget signatures; this
skill only covers what agents commonly get wrong.

Read [the current Slint docs](https://docs.slint.dev/) or open
`https://releases.slint.dev/<version>/docs` for the project's release.
For ~10× fewer tokens `(1.17+)`, swap a page's trailing slash for `.md`: `…/property-types/colors-and-brushes/` → `…/property-types/colors-and-brushes.md`.
That's raw MDX: skip `import` lines; some snippets live in external files.
