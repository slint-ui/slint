# Input Handling, Overlays & Menus

## Input Handling

- **`TouchArea`**: `clicked` is modifier-agnostic; use `pointer-event(ev)` for
  modifier/button-aware handling — `ev.kind == PointerEventKind.down`,
  `ev.button == PointerEventButton.right`, `ev.modifiers.control/.meta/.shift`.
  Also: `double-clicked`, `has-hover`, `pressed`, `mouse-x`/`mouse-y` (local),
  `absolute-position` (window coords), `mouse-cursor`. For right-click *menus*
  use `ContextMenuArea` (below), not `pointer-event`.

## Shortcuts, dialogs & Context Menus

Place a `KeyBinding` directly inside the `FocusScope` that owns the command.
Focus the page when it opens or forward focus to a child in that scope.
Use raw events for input that needs individual presses and releases.

For an in-window confirmation or form, define a reusable `PopupWindow` and instantiate it on the page that opens it.
Call `show()` on the instance.
The popup owns its dismissal, form values, and focus.
For an explicit confirmation, use `no-auto-close` and handle Escape and the buttons inside the popup.
For a dimmed modal, size the popup to the page and place its card over a backdrop inside it.

See the [confirmation popup](../examples/music-player/components/confirm-removal.slint) and its [owning page](../examples/music-player/pages/library.slint).

For context menus, always use the builtin `ContextMenuArea` (no import): it
opens on right-click *and* the keyboard menu key, and its entries are
exposed to accessibility frameworks — a hand-rolled overlay menu is neither.
Set a `MenuBar` directly on the `Window`. `PopupWindow` covers generic
auto-dismiss popups.
