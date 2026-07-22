# Icons & Theming

## Icons

Use `Image` with an SVG asset file:

```slint
import { Palette } from "std-widgets.slint";

component FolderIcon {
    Image {
        source: @image-url("icons/folder.svg");
        width: 24px; height: 24px;
        colorize: Palette.foreground;   // repaint a monochrome icon to match the theme
    }
}
```

SVGs scale cleanly at any size, and `colorize` repaints the whole image with a
brush, so a single monochrome asset works in both color schemes.

If a design specifies glyphs as raw geometry and ships no asset files, **generate
`.svg` files** from it and use them via `Image` — don't hand-draw glyphs as inline
`Path` elements. (`Path` is for shapes you compute or animate, not icon sets.)

## Theming & Light/Dark

- `Palette.color-scheme` (from `std-widgets.slint`) reflects the OS light/dark
  setting and updates live; it's also settable to force a scheme for native
  widgets.
- A clean pattern: one `export global Theme` holding every color/length token
  as `out property`s selected by a `dark` bool, bound to the palette with an
  optional user override:
  ```slint
  import { Palette } from "std-widgets.slint";

  export enum ThemePreference { system, light, dark }

  export global Theme {
      in property <ThemePreference> preference;  // defaults to system
      out property <bool> dark:
          preference == ThemePreference.dark ? true
          : preference == ThemePreference.light ? false
          : Palette.color-scheme == ColorScheme.dark;
      out property <brush> bg: dark ? #1e2025 : #ffffff;
  }

  export component Card inherits Rectangle {
      background: Theme.bg;   // reacts to scheme and preference changes
  }
  ```
  Every component reads `Theme.bg` etc., so theme switching is automatic.
