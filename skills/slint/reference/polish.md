# Visual Polish

A UI that compiles and works can still look rough. Render it — a screenshot or
the MCP server, see [debugging-and-mcp.md](debugging-and-mcp.md) — and review
the image against this list before calling UI work done.

## Spacing & alignment

- Prefer the padded `VerticalBox`/`HorizontalBox` widgets over raw layouts
  with hand-set padding; they give consistent defaults.
- Pick one spacing scale (such as multiples of 8px, halving to 4px where
  tight) and use it for every `spacing`/`padding` — scattered one-off values
  read as untidy.
- Align form labels and fields with a `GridLayout`, not nested boxes with
  eyeballed widths.
- A stretched `Rectangle { }` is a spacer: use stretch factors for centering
  and right-alignment instead of hard-coded gaps.

## Color & type

- Take colors from `Palette` (std-widgets: `background`, `foreground`,
  `alternate-background`, `control-background`, `accent-background`, `border`,
  …) or from a single `Theme` global
  ([icons-and-theming.md](icons-and-theming.md)). Scattered hex literals
  break dark mode and drift out of sync.
- Derive secondary text from the palette instead of picking a gray:
  `Palette.foreground.transparentize(0.4)`.
- Use `rem` for font sizes and a small scale (body, secondary, heading) rather
  than a different `px` value per label.

## Widgets & states

- Prefer std-widgets over hand-rolled controls — keyboard focus, hover states,
  and accessibility come for free and match the platform style.
- Custom interactive elements should react: set `mouse-cursor`, change the
  background on `has-hover`, and `animate` background/opacity over 150–200ms
  so state changes feel intentional rather than abrupt.

## Reviewing a screenshot

Look for: clipped or truncated text, elements touching the window edge,
misaligned baselines, inconsistent gaps between similar items, and overflow
when content is longer than your test data. If the app supports both color
schemes, render both (set the theme via `--load-data`, or
`Palette.color-scheme`).
