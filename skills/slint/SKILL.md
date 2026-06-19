---
name: slint
description: Use when writing, editing, debugging, or reviewing `.slint` UI code, or wiring it to Rust/C++/JS/Python — the Slint language, layout, compile-time gotchas, interop, and the MCP server for runtime checks.
---

# Slint Development Skill

For building, debugging, or reviewing apps that use [Slint](https://slint.dev),
a declarative GUI toolkit for desktop, embedded, mobile, and web.

## Workflow

1. Find the project's Slint version first (`Cargo.toml`/`Cargo.lock`,
   `package.json`, `pyproject.toml`, or the CMake `find_package`/`FetchContent`
   line). This skill targets Slint **≥ 1.17** and its prose assumes the latest
   release; anything that needs a specific version is flagged inline as
   `(1.17+)`, `(1.18+)`, … When the project pins an *older* version, or for
   exact element/property/widget signatures, trust that version's docs over
   this file rather than guessing.
2. After editing: in an IDE with the Slint extension, trust the post-edit
   diagnostics; in a terminal, `slint-viewer --check ui/main.slint` compiles
   one file and prints diagnostics, and `slint-viewer --screenshot` renders it
   (both `(1.17+)`; [debugging-and-mcp.md](reference/debugging-and-mcp.md)).
3. Never declare UI work done without looking at a render — a screenshot for
   appearance, the MCP server for interactions. Review against
   [polish.md](reference/polish.md).
4. Share the render when the host supports it: inline the screenshot in chat
   apps, or print its absolute path and summarize the visual checks in CLI-only
   environments.
5. Offer to run `slint-viewer --auto-reload ui/main.slint` so the user watches
   changes live while you edit.

Most "won't compile" / "won't fill" / "padding ignored" questions are answered
in [gotchas.md](reference/gotchas.md) and
[language-and-layout.md](reference/language-and-layout.md).

## Reference Files

Skim the matching file *before* building in that area, not only when stuck.

| File | Read when… |
|---|---|
| [setup.md](setup.md) | Starting a project / wiring the build (Rust/C++/Node/Python). |
| [language-and-layout.md](reference/language-and-layout.md) | Writing components; an element won't size/fill as expected. |
| [gotchas.md](reference/gotchas.md) | A file won't compile, or colors/units/math/enums behave oddly. |
| [events-and-overlays.md](reference/events-and-overlays.md) | Clicks/keys/modifiers, or popovers/menus/context menus. |
| [icons-and-theming.md](reference/icons-and-theming.md) | Icons, or light/dark theming. |
| [interop.md](reference/interop.md) | Connecting the UI to host-language logic (models, callbacks, globals). |
| [polish.md](reference/polish.md) | The UI works but looks rough; reviewing a rendered screenshot. |
| [debugging-and-mcp.md](reference/debugging-and-mcp.md) | Runtime debugging, headless/CI rendering, screenshots, the MCP server. |
| [tools-install.md](tools-install.md) | Installing `slint-lsp` (language server) or `slint-viewer` (preview / screenshots). |

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

Latest: https://slint.dev/docs. Pin a version with
`https://releases.slint.dev/<version>/docs` (e.g. `…/1.15.1/docs`). Every docs
page also serves its markdown source `(1.17+)` — replace the URL's trailing
slash with `.md` (`…/reference/colors-and-brushes/` →
`…/reference/colors-and-brushes.md`). Prefer it when fetching: it is about 10×
smaller than the HTML. It is raw MDX, so skip the `import` lines, and a few
pages (mostly tutorials) pull code snippets from external files that won't
appear inline. The skill files cover what agents commonly get wrong; the docs
are the authority on element, property, and widget signatures.
