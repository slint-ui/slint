# Icons & Theming

## Icons

Use `Image` with an SVG asset file:

```slint
Image {
    source: @image-url("icons/folder.svg");
    width: 24px; height: 24px;
    colorize: Palette.foreground;   // repaint a monochrome icon to match the theme
}
```

SVGs scale cleanly at any size, and `colorize` repaints the whole image with a
brush, so a single monochrome asset works in both color schemes.

## Theming & Light/Dark

- `Palette.color-scheme` (from `std-widgets.slint`) reflects the OS light/dark
  setting and updates live; it's also settable to force a scheme for native
  widgets.
- A clean pattern: one `export global Theme` holding every color/length token as
  `out property`s selected by a `dark` bool, e.g.
  `out property <brush> bg: dark ? #1e2025 : #ffffff;` Bind `dark` to the palette
  with an optional user override:
  ```slint
  export enum ThemePreference { system, light, dark }

  export global Theme {
      in property <ThemePreference> preference;  // defaults to system
      out property <bool> dark:
          preference == ThemePreference.dark ? true
          : preference == ThemePreference.light ? false
          : Palette.color-scheme == ColorScheme.dark;
  }
  ```
  Every component then reads `Theme.bg` etc., so theme switching is automatic.
