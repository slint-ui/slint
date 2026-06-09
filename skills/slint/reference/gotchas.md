# Gotchas & Common Compile Errors

These bite almost everyone at least once.

- **Units.** `length` (`px`, `pt`, `rem`, …) and `int`/`float` are distinct types.
  Convert with `value * 1px` or `len / 1px`. Slint has no `em` — the compiler
  lists the valid units (`%, phx, px, cm, mm, in, pt, rem, s, ms, deg, grad,
  turn, rad`). For font-size-relative spacing use `rem`, not a hand-converted
  `px` (rewriting `0.04em` → `0.4px` silently breaks scaling).
- **Colors.** Use hex literals (`#rgb`, `#rrggbb`, `#rrggbbaa`) or the color
  functions: `rgb(r,g,b)`, `rgba(r,g,b,a)` (alpha `0.0..1.0`), `hsv(h,s,v[,a])`,
  and **`oklch(l, c, h[, a])`** — so OKLCH design tokens can be used directly,
  e.g. `oklch(0.55, 0.17, 256)`. In `oklch`, `l` is lightness `0..1`, `c` is
  chroma (a number, or a `%` where `100% == 0.4`), `h` is hue in degrees (a number
  or an `angle`), `a` is alpha `0..1`. (There is `hsv` but no `hsl`.) Convert a
  color back with `.to-oklch()` / `.to-hsv()`. Color helpers: `.brighter(f)`,
  `.darker(f)`, `.with-alpha(a)`, `.transparentize(f)`, `.mix(other, f)`; read
  channels with `.red`/`.green`/`.blue`/`.alpha`.
- **Math functions** come in two callable forms (not bare names, generally):
  - methods on a number: `x.floor()`, `x.ceil()`, `x.round()`, `x.sqrt()`,
    `x.mod(y)`, `x.abs()`, `x.clamp(lo, hi)`, `x.max(y)`, `x.min(y)`,
    `x.to-fixed(2)` (→ string), `x.to-precision(3)`;
  - the `Math` namespace: `Math.max(a, b)`, `Math.min`, `Math.clamp`,
    `Math.round`, `Math.floor`, `Math.pow`, `Math.sin`, `Math.atan2`, …
  Use these instead of guessing a bare `floor(...)`. Integer division yields a
  float, so wrap with `.floor()` when assigning to an `int`.
- **Transforms** are available on every element: `transform-rotation` (angle),
  `transform-scale-x` / `transform-scale-y` / `transform-scale` (percent or
  factor), and `transform-origin` (point). They apply visually to the element
  and its descendants; the layout box stays the same.
- **Enum values** are written `EnumName.value`, usually lowercase for builtins:
  `PointerEventKind.down`, `PointerEventButton.right`, `ColorScheme.dark`. Special
  keys use the `Key` namespace with CamelCase: `Key.Return`, `Key.Escape`,
  `Key.UpArrow`, `Key.Backspace`.
- **Gradients**: `@linear-gradient(angle, color stop%, …)`,
  `@radial-gradient(circle, color stop%, …)`, and
  `@conic-gradient(color start-deg, …, color end-deg)`. Radial is
  centered-circle only — it can't be offset/sized like CSS. Repeating patterns
  can be faked with hard color stops.
- **Animations**: `animate width { duration: 200ms; easing: cubic-bezier(0.4,0,0.2,1); }`.
  You animate *properties*, declared inside the element whose property changes.
- **`padding`/`spacing` are ignored on non-layout elements** — see
  `reference/language-and-layout.md`.
- **Only bind a property when you're overriding the default.** Leaving
  `visible`, `enabled`, `clip`, `opacity`, `x`, … unbound lets the compiler
  mark them constant; binding to the default value loses that optimization.
  Inside a layout it's worse: an explicit `x: 0` overrides the computed
  position.

For "element won't fill / is centered" and "padding does nothing", see the Layout
& Sizing section in `reference/language-and-layout.md`.
