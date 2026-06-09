---
name: slint
description: Expert guidance for Slint GUI development — .slint language and layout, common gotchas, Rust/C++/JS/Python interop, and the embedded MCP server for runtime debugging.
---

# Slint Development Skill

Use when building, debugging, or reviewing apps that use [Slint](https://slint.dev),
a declarative GUI toolkit for native UIs across desktop, embedded, mobile, and web.

## When to Use

- Writing or debugging `.slint` files
- Integrating Slint with Rust, C++, JavaScript, or Python
- Layout, binding, rendering, or event-handling issues
- Enabling the Slint MCP server for runtime inspection
- Reviewing Slint-specific code patterns

## How to Help

- Prefer idiomatic Slint patterns; match the user's language binding and version.
- Most "won't compile" / "won't fill" / "padding ignored" questions are answered
  in `reference/gotchas.md` and `reference/language-and-layout.md` — check there.
- Suggest the MCP server when runtime inspection or interaction would help.
- When unsure about an element/property, check the version's docs (below) rather
  than guessing — the API is small and precise.

## Reference Files (read on demand)

This entry point is short by design. Open the relevant file when needed:

| File | Read when… |
|---|---|
| `setup.md` | Starting a project / wiring the build (Rust/C++/Node/Python). |
| `reference/language-and-layout.md` | Writing components; an element won't size/fill as expected. |
| `reference/gotchas.md` | A file won't compile, or colors/units/math/transforms/enums behave oddly. |
| `reference/events-and-overlays.md` | Clicks/keys/modifiers, or popovers/menus/context menus. |
| `reference/drawing-and-theming.md` | Custom vector graphics (`Path`), or light/dark theming. |
| `reference/interop.md` | Connecting the UI to host-language logic (models, callbacks, globals). |
| `reference/debugging-and-mcp.md` | Runtime debugging, headless/CI rendering, screenshots, the MCP server. |
| `tools-install.md` | Installing `slint-lsp` (language server) or `slint-viewer` (preview / screenshots). |

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
interop: `export global Foo { ... }`. One-time code: `init => { ... }`. Details
are in the reference files.

## Documentation Reference

Latest: https://slint.dev/docs — Language guide, Reference (elements, properties,
types, widgets), Language integrations (Rust/C++/Node/Python), Tutorials. Pin a
version with `https://releases.slint.dev/<version>/docs` (e.g. `…/1.15.1/docs`).
Consult these for exact element/property signatures rather than guessing.
