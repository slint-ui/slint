# The `.slint` Language & Layout

## Language Essentials

Declarative and reactive: a property binding re-evaluates automatically when
anything it reads changes.

```slint
import { Button, VerticalBox } from "std-widgets.slint";

// Root element decides the component's nature: a Rectangle fills its parent;
// a layout sizes to its content (see Layout & Sizing).
component Counter inherits Rectangle {
    in property <string> label;          // set by parent / host language
    out property <int> count;            // readable by the parent
    in-out property <bool> enabled: true;// read+write both sides
    private property <int> step: 1;      // internal only
    callback changed(int);

    background: area.has-hover ? #eee : transparent;  // reactive

    VerticalBox {
        Text { text: "\{root.label}: \{root.count}"; }   // interpolation
        Button { text: "+"; clicked => { root.count += root.step; root.changed(root.count); } }
    }
    area := TouchArea { }                 // `name :=` assigns an id
}
```

- **Properties**: `[in|out|in-out|private] property <type> name[: default];`. The
  type goes in angle brackets. Be explicit on anything crossing a component
  boundary.
- **Bindings vs assignments**: `name: expr;` at element scope is a *reactive*
  binding (re-evaluates when inputs change); `name = expr;` inside a callback
  body imperatively assigns. `in` properties can only be read inside the
  component; `out`/`in-out`/`private` can be written.
- **Two-way binding**: `width <=> other.width;`.
- **Callbacks**: `callback foo(int) -> int;` declares; `foo(arg) => { … }`
  handles (named params, not `$0`); `self.foo(1)` invokes. Mark a callback
  `pure` if you want to call it from a property binding — bindings must be
  side-effect free.
- **Functions**: `function name(arg: type) -> type { return expr; }` inside a
  component or global. Mark them `pure function` to call from a property
  binding.
- **Control flow at element scope**: `if cond : Elem { }`;
  `for item[index] in model : Elem { }`; `for i in 5 :` (or `for _ in 5 :`
  when the index is unused) iterates `0..4`. Inside *expressions*, conditionals
  use the ternary `cond ? a : b`.
- **String interpolation**: `"Count: \{root.count}"` — backslash-brace, not
  `${…}` or `{…}`.
- **Element ids**: `name := Element { … }` (no `id` property).
- **`@children`**: forward injected children into a placeholder.
- **Globals**: `export global Foo { ... }` — shared state, theme tokens, host
  interop (see `reference/interop.md`).
- **`init => { ... }`**: runs once on creation (e.g. `some-focus-scope.focus()`).
- **Scope references**: `self` is the current element; `root` is the enclosing
  component's root; `parent` is the parent element. Qualify with `root.foo`
  inside nested elements to target the component root explicitly — bare `foo`
  picks up the closest `foo` in scope.

## Layout & Sizing (read before fighting the layout)

- Use layouts (`VerticalLayout`, `HorizontalLayout`, `GridLayout`, or the padded
  `*Box` widgets); reserve `x`/`y` for overlays, popovers, and custom drawing.
- **`padding`/`spacing` only work on layout elements.** On a `Text`/`Rectangle`
  they're silently ignored (the compiler warns). To inset a `Text`, wrap it:
  `HorizontalLayout { padding-left: 6px; Text {...} }`.
- **Fill vs. preferred size.** Most containers and graphics elements
  (`Rectangle`, `TouchArea`, `FocusScope`, `Flickable`, `Path`, and every layout
  — `VerticalLayout` / `HorizontalLayout` / `GridLayout` / `VerticalBox` / …)
  fill their parent by default. `Text` and `Image` take their *preferred*
  (implicit) size. A custom component inherits the behavior of its root element:
  `component Card inherits Rectangle { … }` fills, but
  `component Label inherits Text { … }` doesn't. To make a non-filling element
  stretch, set `width: 100%; height: 100%`, change the component's root to a
  filling element, or place it inside a layout.
- **Default position outside a layout is centered.** An element with implicit
  size and no explicit `x`/`y` sits at `(parent.width - self.width) / 2` (and
  the same for `y`). Use a layout if you want top-left placement, or set
  `x: 0; y: 0;`.
- Distribute space with `horizontal-stretch`/`vertical-stretch` and
  `min/preferred/max-width/height`; a stretched `Rectangle { }` is a flexible
  spacer.
- Z-order: later siblings render on top.
- An app's top-level exported element should be `Window` or `Dialog`. Other
  roots compile, but the compiler emits a deprecation warning.
