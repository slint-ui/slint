# Input Handling, Overlays & Menus

## Input Handling

- **`TouchArea`**: `clicked` is modifier-agnostic; use `pointer-event(ev)` for
  modifier/button-aware handling — `ev.kind == PointerEventKind.down`,
  `ev.button == PointerEventButton.right`, `ev.modifiers.control/.meta/.shift`.
  Also: `double-clicked`, `has-hover`, `pressed`, `mouse-x`/`mouse-y` (local),
  `absolute-position` (window coords), `mouse-cursor`. For right-click *menus*
  use `ContextMenuArea` (below), not `pointer-event`.
## Shortcuts and dialogs

Give each shortcut to the page or dialog where it applies.
A `KeyBinding` is a direct child of a `FocusScope`; focus the page when it opens or forward focus to a child in that scope.
Use raw key events for input that needs individual presses and releases.

```slint
export component LibraryPage inherits FocusScope {
    init => { self.focus(); }
    KeyBinding {
        keys: @keys(Control + F);
        activated => { search-field.focus(); }
    }
    search-field := LineEdit { }
}
```

For an in-window confirmation or form, define a reusable `PopupWindow` and instantiate it on the page that opens it.
Call `show()` on the instance.
The popup owns its dismissal, form values, and focus.
For an explicit confirmation, use `no-auto-close` and handle Escape and the buttons inside the popup.
For a dimmed modal, size the popup to the page and place its card over a backdrop inside it.

```slint
export component ConfirmRemoval inherits PopupWindow {
    callback confirmed();
    close-policy: no-auto-close;
    FocusScope {
        init => { self.focus(); }
        KeyBinding {
            keys: @keys(Escape);
            activated => { root.close(); }
        }
        HorizontalLayout {
            Button { text: "Cancel"; clicked => { root.close(); } }
            Button { text: "Remove"; clicked => { root.confirmed(); root.close(); } }
        }
    }
}
```

Import `Button` and `LineEdit` from `"std-widgets.slint"` for these examples.
Use `ContextMenuArea` for context menus and `MenuBar` directly on the window for application menus.
