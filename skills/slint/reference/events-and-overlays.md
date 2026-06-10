# Input Handling, Overlays & Menus

## Input Handling

- **`TouchArea`**: `clicked` is modifier-agnostic; use `pointer-event(ev)` for
  modifier/button-aware handling — `ev.kind == PointerEventKind.down`,
  `ev.button == PointerEventButton.right`, `ev.modifiers.control/.meta/.shift`.
  Also: `double-clicked`, `has-hover`, `pressed`, `mouse-x`/`mouse-y` (local),
  `absolute-position` (window coords), `mouse-cursor`. For right-click *menus*
  use `ContextMenuArea` (below), not `pointer-event`.
- **`FocusScope`** for keys (see the
  [key event delivery docs](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/focusscope/#key-event-delivery)):
  `key-pressed` on an enclosing scope sees only keys the focused child
  *rejected*; `capture-key-pressed` runs *before* the focused child — use it
  for shortcuts a `TextInput` would otherwise consume (e.g. Ctrl+A). Handlers
  return `accept`/`reject`. Set initial focus declaratively with
  `forward-focus`, and nest inputs and widgets *inside* the scope so it takes
  part in delivery — that nesting, not an imperative `fs.focus()` on
  background clicks, is the fix for "shortcuts stopped working after I
  clicked the input".

## Overlays, Popovers & Context Menus

- For context menus, always use the builtin `ContextMenuArea` (no import): it
  opens on right-click *and* the keyboard menu key, and its entries are
  exposed to accessibility frameworks — a hand-rolled overlay menu is neither.
  Set a `MenuBar` directly on the `Window`. `PopupWindow` covers generic
  auto-dismiss popups.
- For exact positioning of a non-menu popover (e.g. a button-anchored panel),
  a manual overlay works:
  - Render it as a child of the top-level `Window`, gated by `if open : …`.
    `absolute-position` is window-local, so under any other root subtract the
    overlay parent's `absolute-position` to convert.
  - Add a full-window backdrop `TouchArea` behind it to close on click.
  - Anchor with `widget.absolute-position.x/.y` (+ height), clamping both
    edges: `x.clamp(8px, root.width - panel.width - 8px)`.
- A panel placed directly under `Window` defaults to *filling* the window —
  set `height: layout.preferred-height;` to size it to its content (see
  fill-vs-preferred in [language-and-layout.md](language-and-layout.md)).
