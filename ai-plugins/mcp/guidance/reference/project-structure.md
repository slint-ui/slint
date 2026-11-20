# Structure a Slint application

For a multi-page UI, organize the Slint source around the parts a developer changes together.
A typical UI directory looks like this:

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

Start a refactor by following one interaction from the widget to its owner.
The window selects a page with an enum.
The page owns its commands, shortcuts, and popup instances.
The component owns transient state such as expansion or draft text.
Only state shared with the host or across views needs a global.
Group it by feature, and re-export each host-facing global from main.slint.

```slint
// pages/player.slint
export enum RepeatMode { off, track, playlist }
export global Player {
    in property <RepeatMode> repeat-mode;
    callback change-repeat(mode: RepeatMode);
}

export component PlayerPage inherits Rectangle {
    Button {
        text: @tr("Repeat track");
        clicked => { Player.change-repeat(RepeatMode.track); }
    }
}
```

Import `Button` from `"std-widgets.slint"`.
Use an enum for a closed set of choices and named callbacks for commands.
Use a struct to keep a model item's identity, label and image together.
When the interface may change, replace numeric modes and string dispatch at their source.
Moving them into a window adapter leaves every caller coupled to the old protocol.

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
