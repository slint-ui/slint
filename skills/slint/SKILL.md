---
name: slint
description: Expert guidance for Slint GUI development — .slint language and layout, common gotchas, Rust/C++/JS/Python interop, and the embedded MCP server for runtime debugging.
---

# Slint Development Skill

For building, debugging, or reviewing apps that use [Slint](https://slint.dev),
a declarative GUI toolkit for desktop, embedded, mobile, and web.

## Workflow

1. Match the project's Slint version (`Cargo.toml`/`Cargo.lock`,
   `package.json`, `pyproject.toml`, or the CMake `find_package`/`FetchContent`
   line) and consult that version's docs for exact APIs rather than guessing.
2. After editing: in an IDE with the Slint extension, trust the post-edit
   diagnostics; in a terminal, `slint-viewer --check ui/main.slint` compiles
   one file and prints diagnostics, and `slint-viewer --screenshot` renders it
   ([debugging-and-mcp.md](reference/debugging-and-mcp.md)).
3. Never declare UI work done without looking at a render — a screenshot for
   appearance, the MCP server for interactions. Review against
   [polish.md](reference/polish.md).
4. Offer to run `slint-viewer --auto-reload ui/main.slint` so the user watches
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
`https://releases.slint.dev/<version>/docs` (e.g. `…/1.15.1/docs`). From 1.17,
every docs page also serves its markdown source — replace the URL's trailing
slash with `.md` (`…/reference/colors-and-brushes/` →
`…/reference/colors-and-brushes.md`). Prefer it when fetching: it is about 10×
smaller than the HTML. It is raw MDX, so skip the `import` lines, and a few
pages (mostly tutorials) pull code snippets from external files that won't
appear inline. The skill files cover what agents commonly get wrong; the docs
are the authority on element, property, and widget signatures.
