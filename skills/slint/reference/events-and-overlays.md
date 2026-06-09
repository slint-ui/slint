# Input Handling, Overlays & Menus

## Input Handling

- **`TouchArea`**: `clicked => {}` is modifier-agnostic. For modifier/button-aware
  handling use `pointer-event(ev)`:
  ```slint
  TouchArea {
      pointer-event(ev) => {
          if (ev.kind == PointerEventKind.down) {
              if (ev.button == PointerEventButton.right) { debug("context menu"); }
              if (ev.modifiers.control || ev.modifiers.meta) { debug("multi-select"); }
              if (ev.modifiers.shift) { debug("range select"); }
          }
      }
      double-clicked => { debug("open"); }
  }
  ```
  Other members: `has-hover`, `pressed`, `mouse-x`/`mouse-y` (local),
  `absolute-position` (window coords), `mouse-cursor`.
- **`FocusScope`** for keys — it handles a key event when it has focus *or when
  it surrounds the focused element*. Give it focus declaratively with
  `forward-focus`, and place `TextInput`s and widgets inside it so unhandled
  keys bubble up to your `key-pressed`. The handler returns `accept`/`reject`:
  ```slint
  export component App inherits Window {
      forward-focus: fs;   // fs gets focus when the window is activated
      fs := FocusScope {
          key-pressed(e) => {
              if (e.text == Key.Escape) { return accept; }
              if ((e.modifiers.control || e.modifiers.meta) && e.text == "a") { return accept; }
              return reject;
          }
          // TextInputs / widgets go HERE so the FocusScope surrounds them.
      }
  }
  ```
  Reach for `fs.focus()` only as an imperative escape hatch — the idiomatic fix
  for "shortcuts stopped working after I clicked the input" is to nest the
  input inside the FocusScope, not to refocus on background click.

## Overlays, Popovers & Context Menus

- `PopupWindow` covers the common cases with auto-dismiss. For menus, use the
  builtin `ContextMenuArea` (no import) and set a `MenuBar` directly on the
  `Window`. For exact positioning (cursor-anchored menu, button-anchored
  dropdown), use a manual overlay:
  - Render it as a child of the top-level `Window`, gated by `if open : …`.
    `absolute-position` is window-local, so this trick only works when the
    overlay sits under the actual top-level `Window`. Under any other root,
    subtract the overlay parent's `absolute-position` to convert.
  - Add a full-window backdrop `TouchArea` behind it to close on click.
  - Anchor with `widget.absolute-position.x/.y` (+ height), clamping both edges:
    `x.clamp(8px, root.width - menu.width - 8px)`.
- A custom popover panel (a `Rectangle` directly under `Window`) defaults to
  filling the window — set `height: layout.preferred-height;` so it sizes to its
  content (see fill-vs-preferred in `reference/language-and-layout.md`).
