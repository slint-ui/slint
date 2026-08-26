# Window & Backend Integration

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/core/window.rs`,
> `internal/core/platform.rs`, `internal/backends/`, window management,
> platform integration, or implementing custom backends.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint's window system provides an abstraction layer between the UI framework and platform windowing systems. It consists of:

- **Window API**: Public interface for window operations
- **WindowAdapter trait**: Backend implementation interface
- **Platform trait**: Backend factory and event loop
- **WindowEvent enum**: Events from windowing system to Slint
- **WindowInner**: Internal state management

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/window.rs` | WindowInner, WindowAdapter trait |
| `internal/core/platform.rs` | Platform trait, WindowEvent enum |
| `internal/core/window/popup.rs` | Popup placement (`Placement`, `place_popup`) — `PopupWindow` itself is in `window.rs` |
| `internal/backends/winit/` | Winit-based cross-platform backend |
| `internal/backends/qt/` | Qt integration backend |
| `internal/backends/linuxkms/` | Direct Linux KMS rendering |
| `internal/backends/android-activity/` | Android activity backend |
| `internal/backends/testing/` | Testing/headless backend |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User Application                          │
├─────────────────────────────────────────────────────────────┤
│                    Window (Public API)                       │
│  - show(), hide(), set_size(), set_position()               │
│  - request_redraw(), dispatch_event()                       │
├─────────────────────────────────────────────────────────────┤
│                    WindowInner                               │
│  - Component management, focus, popups                      │
│  - Mouse/keyboard input processing                          │
│  - Property tracking for redraw/updates                     │
├─────────────────────────────────────────────────────────────┤
│                    WindowAdapter (trait)                     │
│  - Platform-specific window implementation                  │
│  - Renderer integration                                     │
├─────────────────────────────────────────────────────────────┤
│                    Platform (trait)                          │
│  - Window creation, event loop                              │
│  - Clipboard, timers, duration                              │
├─────────────────────────────────────────────────────────────┤
│              Platform Backend (winit, Qt, etc.)              │
└─────────────────────────────────────────────────────────────┘
```

## WindowAdapter Trait

The main interface backends must implement. Only three methods have no default: `window()`
returns the public `Window`, `size()` the current size in physical pixels excluding the frame,
and `renderer()` the renderer to draw with.

The rest are optional: `set_visible()`, `position()` / `set_position()`, `set_size()`,
`request_redraw()`, `update_window_properties()` (title, constraints, ... — see
[Window Properties](#window-properties)), `internal()` to expose the extra trait below, and
`window_handle_06()` / `display_handle_06()` for raw-window-handle interop.
See `WindowAdapter` in `internal/core/window.rs`.

### WindowAdapterInternal

Additional internal methods, reached through `WindowAdapter::internal()` and not part of the
public API. Every one has a default, so a backend implements only what it supports:

- `register_item_tree()` / `unregister_item_tree()` when a component tree appears or goes away
- `get_parent()` and `create_child_window_adapter()`, the latter returning a separate top-level
  window for a popup, or `None` to render it embedded in the parent
- `set_mouse_cursor()`, `input_method_request()`, `handle_focus_change()` (for accessibility)
- `supports_native_menu_bar()`, `setup_menubar()`, `show_native_popup_menu()`
- `window_handle_06_rc()` / `display_handle_06_rc()`, `bring_to_front()`, `start_window_move()`,
  `start_drag()`
- `safe_area_inset()` for notches and system bars

See `WindowAdapterInternal` in `internal/core/window.rs`.

## Platform Trait

Factory for windows and event loop management. `create_window_adapter()` is the only required
method. The rest have defaults:

- `run_event_loop()` runs the loop until it is asked to quit. `process_events()` handles the
  pending events and waits for new ones up to a timeout, for an application driving the loop
  itself; it is `#[doc(hidden)]` and takes an `InternalToken`, and a backend implements the two
  separately — neither default calls the other.
- `new_event_loop_proxy()` returns the handle for cross-thread communication. Its
  `quit_event_loop()` exits the loop; there is no `quit_event_loop` directly on `Platform`.
  (`set_event_loop_quit_on_last_window_closed()` is hidden and deprecated — i-slint-core owns
  that behavior now.)
- `clipboard_text()` / `set_clipboard_text()`, both taking which `Clipboard` to use.
- `duration_since_start()` drives the animations, `click_interval()` the double-click detection,
  and `cursor_flash_cycle()` the text cursor blink.
- `debug_log()` receives the output of Slint's `debug()`, and `open_url()` hands a URL to an
  external browser.

See `Platform` in `internal/core/platform.rs`.

## WindowEvent

Events dispatched from platform to Slint. `WindowEvent` has:

- **Pointer events**: `PointerPressed` and `PointerReleased` (a logical position and a
  `PointerEventButton`), `PointerMoved`, `PointerScrolled` (a position plus an x and y delta) and
  `PointerExited`.
- **Keyboard events**: `KeyPressed`, `KeyPressRepeated` and `KeyReleased`, each carrying the text
  the key produced.
- **Window state events**: `ScaleFactorChanged`, `Resized` (a logical size), `CloseRequested` and
  `WindowActiveChanged`.

See `WindowEvent` in `internal/core/platform.rs`.

There is no public touch variant. A backend with real touch input dispatches
`WindowEvent::Internal(InternalEvent::Touch { .. })` instead, because the pointer events the
runtime synthesizes from a touch point carry a finger id that `WindowEvent` cannot express.

**Dispatching events:**
```rust
// From platform backend to Slint
window.dispatch_event(WindowEvent::PointerPressed {
    position: LogicalPosition::new(100.0, 50.0),
    button: PointerEventButton::Left,
});
```

## WindowInner

`WindowInner` (`internal/core/window.rs`) holds everything the public `Window` does not. Its
fields group into:

- The weak adapter it belongs to, the shown component, and a strong reference to it kept only
  while the window is visible.
- **Input state**: the mouse input state, the touch state, and a `ClickState` for double-click
  detection.
- **Focus**: the focused item and a tracker for its visibility, the text cursor blinker, the last
  text sent to the input method, and a `prevent_focus_change` flag that keeps a
  `ComponentContainer`'s init code from stealing the focus.
- **Property tracking**: `WindowPinnedFields`, the pinned block holding `scale_factor`, `active`,
  `text_input_focused`, `menubar_shortcuts` and the redraw / window-properties trackers.
- **Popups**: the stack of active popups, the id to hand out next, and whether one was open at
  the last press.
- The menu bar, the `close_requested` callback, the `SlintContext`, and the native drag in flight.

### Property Tracking

Windows use `PropertyTracker`s parameterized on a `PropertyDirtyHandler`, so a dependency going
dirty calls straight into the window. Both handlers hold a weak adapter: `WindowRedrawTracker`
calls `request_redraw()` as soon as a rendered property goes dirty, and
`WindowPropertiesTracker` defers `update_window_properties()` to a single-shot timer so a burst of
changes results in one update. `PopupWindowPropertiesTracker` does the same for a popup's
geometry. See `internal/core/window.rs`.

## Popup Management

### PopupWindow Structure

`PopupWindow` and `PopupWindowLocation` are defined in `internal/core/window.rs`;
`PopupClosePolicy` is defined in `internal/common/enums.rs` and re-exported via
`crate::items::PopupClosePolicy`.

A `PopupWindow` is its id, its `PopupWindowLocation`, the component providing the content, the
`PopupClosePolicy`, the item it was invoked from, the item that had the focus in the parent
window, a `WindowKind` (a tooltip does not steal focus and is placed unclamped, while a context
or popup menu joins the menu chain for hit testing and cascading close), a closure returning the
popup's desired position relative to the parent, a hook keeping the parent's
`PopupWindow::is-open` property in sync, and its own properties tracker.

A `PopupWindowLocation` is either `TopLevel`, its own window known to the windowing system, or
`ChildWindow` at a position inside the parent.

`PopupClosePolicy` is `CloseOnClick` (any click), `CloseOnClickOutside`, or `NoAutoClose`.

### Popup Placement

`place_popup()` takes a `Placement` — currently only `Fixed`, a requested rectangle — and an
optional clip region, typically the window or the screen it is on, and returns where the popup
actually goes. See `internal/core/window/popup.rs`.

The placement algorithm:
1. If popup fits within clip region, use requested position
2. Otherwise, clamp position to keep popup visible
3. If popup is larger than clip region, shrink to fit

## Available Backends

### Winit Backend (`internal/backends/winit/`)

Cross-platform backend using the winit library:

- **Platforms**: Windows, macOS, Linux (X11/Wayland), iOS, Android, WASM
- **Renderers**: FemtoVG (OpenGL/WGPU), Skia, Software
- **Features**: Accessibility (AccessKit), menus (muda)

Renderers plug in through the `WinitCompatibleRenderer` trait in
`internal/backends/winit/lib.rs`; see
[custom-renderer.md](custom-renderer.md#winitcompatiblerenderer-internalbackendswinit).

### Qt Backend (`internal/backends/qt/`)

Native Qt integration:

- Native styling and widgets
- Qt event loop integration
- Platform dialogs (file, color, etc.)

### Linux KMS Backend (`internal/backends/linuxkms/`)

Direct framebuffer rendering:

- No windowing system required
- DRM/KMS for display
- libinput for input

### Testing Backend (`internal/backends/testing/`)

Headless testing:

- No actual rendering
- Simulated input
- Automated UI testing

## Window Properties

Properties exposed to backends via `WindowProperties`:

`WindowProperties` borrows the `WindowInner` and exposes getters for the `Window` element's
properties: `title()`, `background()`, `layout_constraints()`, and `is_fullscreen()`,
`is_maximized()`, `is_minimized()`. `LayoutConstraints` here is the window-level one — an
optional min and max plus a preferred `LogicalSize` — not the compiler's.
See `internal/core/window.rs`.

## Input Method Support

For text input with IME. An `InputMethodRequest` is `Enable` or `Update` with the properties, or
`Disable`.

`InputMethodProperties` carries the text surrounding the cursor (pre-edit excluded), the cursor
byte offset within it, the selection anchor if there is one, the pre-edit text and its offset, the
cursor rectangle's origin and size, the clip rectangle, the anchor point, the `InputType` (text,
number, password, ...) and the `InputMethodHints`.
See `internal/core/window.rs`.

## Common Patterns

### Implementing a Minimal WindowAdapter

```rust
struct MyWindowAdapter {
    window: Window,
    renderer: SoftwareRenderer,
    size: Cell<PhysicalSize>,
}

impl WindowAdapter for MyWindowAdapter {
    fn window(&self) -> &Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        // Schedule redraw in your event loop
    }
}
```

### Dispatching Events

```rust
// Window resize
window.dispatch_event(WindowEvent::Resized {
    size: LogicalSize::new(800.0, 600.0),
});

// Scale factor change (important for DPI)
window.dispatch_event(WindowEvent::ScaleFactorChanged {
    scale_factor: 2.0,
});

// Mouse input
window.dispatch_event(WindowEvent::PointerMoved {
    position: LogicalPosition::new(x, y),
});

// Keyboard input (using Key enum)
window.dispatch_event(WindowEvent::KeyPressed {
    text: slint::platform::Key::Return.into(),
});
```

### Handling Close Request

```rust
// In platform backend
window.dispatch_event(WindowEvent::CloseRequested);

// In application
window.on_close_requested(|| {
    if has_unsaved_changes() {
        CloseRequestResponse::KeepWindowShown
    } else {
        CloseRequestResponse::HideWindow
    }
});
```

## Coordinate Systems

| Type | Description |
|------|-------------|
| **Physical** | Actual screen pixels |
| **Logical** | DPI-independent pixels (physical / scale_factor) |

```rust
let logical = physical_size.to_logical(scale_factor);
let physical = logical_size.to_physical(scale_factor);
```

The window API uses both: `WindowAdapter::position()` and `size()` are physical, while
`set_size()` takes a `WindowSize` — either `Physical` or `Logical` — and `set_position()` a
`WindowPosition` the same way.

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| No rendering | Missing request_redraw | Call request_redraw after changes |
| Wrong size | Scale factor not set | Dispatch ScaleFactorChanged event |
| Input not working | Events not dispatched | Check dispatch_event calls |
| Window not updating | PropertyTracker not triggering | Check component is set |
| Popup in wrong place | Coordinate system mismatch | Use logical coordinates |

### Checking Window State

```rust
// Get current focus
let focus = WindowInner::from_pub(&window).focus_item.borrow().clone();

// Check scale factor
let scale = WindowInner::from_pub(&window).scale_factor();

// Check active popups
let popups = WindowInner::from_pub(&window).active_popups();
```

## Testing

```sh
# Run window tests
cargo test -p i-slint-core window

# Run backend-specific tests
cargo test -p i-slint-backend-winit
cargo test -p i-slint-backend-qt

# Run with testing backend
cargo test -p i-slint-backend-testing
```
