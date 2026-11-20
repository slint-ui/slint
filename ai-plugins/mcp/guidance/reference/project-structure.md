# Structure a Slint application

When refactoring a UI, first sketch the new entry file and the public interface of each page.
If the caller permits an interface change, design the Slint API for the target release.
An older host can migrate separately. Validate the UI by opening its project in the target viewer.

A typical multi-page UI has this structure.

```text
main.slint                # Window and page selection
slint-project.json        # {"main": "main.slint", "edition": "1.19"}
style.slint               # Style: named brushes and colors
metrics.slint             # Metrics: shared spacing, sizes and typography
pages/
  index.slint
  library.slint
  player.slint
components/
  index.slint
  album-tile.slint
  confirmation-dialog.slint
assets/
  icons.slint             # Icons global pointing into icons/
  images.slint            # Images global pointing into images/
  icons/
  images/
```

Use the project file on Slint 1.19+ and create asset groups as you need them.
Keep SVG, PNG, and WebP files under their asset group and other media such as videos under assets/.
An index re-exports the public names from its folder, for example `export { AlbumTile } from "album-tile.slint";`.
Keep reusable visual components in their own files.
A page can keep small private helpers beside their only caller.

## Decide who owns each interaction

Keep the window small enough to see the complete navigation at a glance:

```slint
// main.slint
import { LibraryPage, PlayerPage } from "pages/index.slint";
export { Library, Player } from "pages/index.slint";
export enum Page { library, player }

export component MainWindow inherits Window {
    in-out property <Page> page: Page.library;
    if root.page == Page.library: LibraryPage { }
    if root.page == Page.player: PlayerPage { }
}
```

Put each host-facing global beside its page and re-export it through both index and entry files.
A page invokes its own named commands directly.
Keep draft text in the form, expansion in the row, and each popup instance on its owning page.

```slint
// pages/player.slint
export enum RepeatMode { off, track, playlist }
export global Player {
    in property <RepeatMode> repeat-mode;
    callback change-repeat(mode: RepeatMode);
}
```

For example, bind a button's `clicked` to `Player.change-repeat(RepeatMode.track)`.
Use a struct to keep each model item's identity, label and image together.

Review the result starting at main.slint.
If it still forwards page properties, switches on dialog numbers, or translates commands into strings, finish moving those decisions to their owners.
Retain a legacy adapter only when the caller requires API compatibility.

## Share design decisions

```slint
// metrics.slint
export global Metrics {
    out property <length> gap: 8px;
    out property <length> panel-padding: 2 * gap;
}
```

Use `padding: Metrics.panel-padding;` and `spacing: Metrics.gap;` throughout the layouts that share those measurements.
Give recurring icon sizes, radii, and text sizes the same treatment.
Let content and layout stretch determine panel sizes.
For popup and shortcut examples, see [input handling](events-and-overlays.md).
