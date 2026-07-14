# The `.slint` Language & Layout

Syntax primer: the example in SKILL.md. Full guide: the docs' language section.
Below are the rules agents most often get wrong.

## Language

- Properties: `[in|out|in-out|private] property <type> name[: default];` — be
  explicit on anything crossing a component boundary.
- `name: expr;` at element scope is a *reactive* binding; `name = expr;` inside
  a callback assigns imperatively. `in` properties can't be written from inside
  the component.
- Two-way binding: `a <=> b`. Callbacks: `callback foo(int) -> int;` declares,
  `foo(arg) => { … }` handles (named params, not `$0`). Functions:
  `function name(a: type) -> type { … }`. Mark callbacks/functions `pure` to
  call them from a binding — bindings must be side-effect free.
- Element control flow: `if cond : Elem {}`; `for item[index] in model : Elem {}`
  (`for i in 5 :` iterates `0..4`). Inside expressions use the ternary
  `cond ? a : b`.
- Ids: `name := Element { … }` (no `id` property). `@children` forwards
  injected children. `export global Foo { … }` holds shared state
  ([interop.md](interop.md)). `init => { … }` runs once on creation.
- `self` is the current element, `root` the component root, `parent` the
  parent element. Bare `foo` resolves to the *nearest* `foo` in scope —
  qualify with `root.` to be explicit.

## Layout & Sizing (read before fighting the layout)

- Use layouts (`VerticalLayout`, `HorizontalLayout`, `GridLayout`, or the
  padded `*Box` widgets); reserve `x`/`y` for overlays and custom drawing.
- `padding`/`spacing` only work on layout elements. To inset a `Text`, wrap
  it: `HorizontalLayout { padding-left: 6px; Text {…} }`.
- Fill vs. preferred size: containers and graphics elements (`Rectangle`,
  `TouchArea`, `FocusScope`, every layout, …) fill their parent by default;
  `Text` and `Image` take their preferred size. A custom component inherits
  its root element's behavior. To stretch a non-filling element set
  `width: 100%; height: 100%`, or place it in a layout.
- Outside a layout, an element with implicit size and no `x`/`y` is *centered*
  in its parent. Set `x: 0; y: 0;` for top-left.
- Distribute space with `horizontal-stretch`/`vertical-stretch` and
  `min/preferred/max-width/height`; a stretched `Rectangle { }` is a spacer.
- Z-order: later siblings render on top.
- The app's top-level exported component should inherit `Window` or `Dialog`.
