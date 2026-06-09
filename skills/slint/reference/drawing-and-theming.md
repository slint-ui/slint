# Custom Drawing & Theming

## Custom Drawing with `Path`

`Path` renders vector graphics from an SVG-style command string — great for icons.

```slint
Path {
    width: 24px; height: 24px;
    viewbox-width: 24; viewbox-height: 24;   // command coordinate space
    commands: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z";
    stroke: black; stroke-width: 1.2px; fill: transparent;
}
```

- `commands` accepts the SVG path mini-language including arcs (`a`/`A`); multiple
  sub-paths (multiple `M`) are fine. Lines/rects/circles must be expressed as path
  commands (a circle ≈ two `a` arcs).
- The path is scaled to fit `width`×`height` using the viewbox, but **`stroke-width`
  is a logical length and is *not* scaled by the viewbox.** To mimic an SVG whose
  stroke is `S` in a `V`-unit viewbox rendered at size `D`, set
  `stroke-width: Spx * D / V` (e.g. `1.6px * size / 24px`). The `1.6px` factor
  gives the expression a unit so it matches the `length`-typed property.
- A single `Path` has one `fill` and one `stroke`. For a glyph that mixes filled
  and stroked sub-shapes, use two overlapping `Path`s (one stroked, one filled).
- Codegen tip: when you have many SVG icons, generate the `commands` strings
  (and a `name → commands` lookup function in a global) from your source rather
  than hand-translating.

## Theming & Light/Dark

- `Palette.color-scheme` (from `std-widgets.slint`) reflects the OS light/dark
  setting and updates live; it's also settable to force a scheme for native
  widgets.
- A clean pattern: one `export global Theme` holding every color/length token as
  `out property`s selected by a `dark` bool, e.g.
  `out property <brush> bg: dark ? #1e2025 : #ffffff;` Bind `dark` to the palette
  with an optional user override:
  ```slint
  out property <bool> dark:
      pref == 2 ? true
      : pref == 1 ? false
      : Palette.color-scheme == ColorScheme.dark;
  ```
  Every component then reads `Theme.bg` etc., so theme switching is automatic.
