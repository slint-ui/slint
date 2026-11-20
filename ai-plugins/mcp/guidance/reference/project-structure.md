# Structure a Slint application

When refactoring a UI, first sketch the new entry file and the public interface of each page.
If the caller permits an interface change, design the Slint API for the target release.
An older host can migrate separately.
Validate the UI by opening its project in the target viewer.

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

Keep navigation visible at a glance.

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

Define each host-facing global in its page file and re-export it through the page index and main.slint.
Here the keyboard and the button open the same page-owned confirmation.
Reset draft fields on opening when each invocation starts a new edit.

```slint
// pages/library.slint
import { Button } from "std-widgets.slint";
import { ConfirmRemoval } from "../components/index.slint";
import { Metrics } from "../metrics.slint";

export global Library {
    callback remove-selection();
}

export component LibraryPage inherits FocusScope {
    init => { self.focus(); }
    KeyBinding {
        keys: @keys(Delete);
        activated => { confirm.show(); }
    }
    HorizontalLayout {
        padding: Metrics.panel-padding;
        spacing: Metrics.gap;
        alignment: end;
        Button { text: @tr("Remove…"); clicked => { confirm.show(); } }
    }
    confirm := ConfirmRemoval {
        confirmed => { Library.remove-selection(); }
    }
}
```

Implement [ConfirmRemoval as a PopupWindow](events-and-overlays.md#shortcuts-and-dialogs).
Give each workflow its own instance, content and confirmation callback.
A single dialog enum and an application-wide switch on confirm or Escape defeat that ownership.
Keep draft text in the form and expansion in the row.
For closed choices use an enum such as `RepeatMode { off, track, playlist }`.
Pass the enum to a named callback.
Keep a model item's identity, label, and image together in a struct.

Review the refactor by tracing one button and its shortcut.
Both should reach the same page operation without a round trip through the window.
Retain a legacy adapter only when the caller requires API compatibility.

## Share design decisions

```slint
// metrics.slint
export global Metrics {
    out property <length> gap: 8px;
    out property <length> panel-padding: 2 * gap;
}
```

Replace recurring measurements at their use sites, including padding, spacing, radii, icon sizes, and typography.
Arrange floating sidebars with a horizontal layout over the background and a stretching spacer between panels.
Use a vertical layout for stacked panels so resizing keeps the same gaps.
