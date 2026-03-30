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
| `internal/core/window/popup.rs` | Popup placement and management |
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

The main interface backends must implement:

```rust
pub trait WindowAdapter {
    /// Returns the window API object
    fn window(&self) -> &Window;

    /// Show or hide the window
    fn set_visible(&self, visible: bool) -> Result<(), PlatformError>;

    /// Get window position (physical screen coordinates)
    fn position(&self) -> Option<PhysicalPosition>;

    /// Set window position
    fn set_position(&self, position: WindowPosition);

    /// Get window size (physical pixels, excluding frame)
    fn size(&self) -> PhysicalSize;

    /// Set window size
    fn set_size(&self, size: WindowSize);

    /// Request asynchronous redraw
    fn request_redraw(&self);

    /// Return the renderer
    fn renderer(&self) -> &dyn Renderer;

    /// Update window properties (title, constraints, etc.)
    fn update_window_properties(&self, properties: WindowProperties<'_>);
}
```

### WindowAdapterInternal

Additional internal methods (not public API):

```rust
pub trait WindowAdapterInternal {
    /// Called when component tree is created
    fn register_item_tree(&self);

    /// Called when component tree is destroyed
    fn unregister_item_tree(&self, component: ItemTreeRef, items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>);

    /// Create a separate window for popup (or None for embedded)
    fn create_popup(&self, geometry: LogicalRect) -> Option<Rc<dyn WindowAdapter>>;

    /// Set the mouse cursor
    fn set_mouse_cursor(&self, cursor: MouseCursor);

    /// Handle input method requests
    fn input_method_request(&self, request: InputMethodRequest);

    /// Handle focus change (for accessibility)
    fn handle_focus_change(&self, old: Option<ItemRc>, new: Option<ItemRc>);

    /// Get the color scheme (light/dark)
    fn color_scheme(&self) -> ColorScheme;

    /// Returns safe area insets (for notches, system bars)
    fn safe_area_inset(&self) -> PhysicalEdges;
}
```

## Platform Trait

Factory for windows and event loop management:

```rust
pub trait Platform {
    /// Create a new window adapter
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError>;

    /// Run the event loop (blocking)
    fn run_event_loop(&self) -> Result<(), PlatformError>;

    /// Run event loop for specified duration
    fn run_event_loop_until_quit(
        &self,
        timeout: Option<Duration>,
    ) -> Result<EventLoopQuitBehavior, PlatformError>;

    /// Exit the event loop
    fn quit_event_loop(&self) -> Result<(), PlatformError>;

    /// Get event loop proxy for cross-thread communication
    fn event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>>;

    /// Get clipboard contents
    fn clipboard_text(&self, clipboard: Clipboard) -> Option<SharedString>;

    /// Set clipboard contents
    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard);

    /// Duration since application start (for animations)
    fn duration_since_start(&self) -> Duration;

    /// Click interval for double-click detection
    fn click_interval(&self) -> Duration;
}
```

## WindowEvent

Events dispatched from platform to Slint:

```rust
pub enum WindowEvent {
    // Pointer events
    PointerPressed { position: LogicalPosition, button: PointerEventButton },
    PointerReleased { position: LogicalPosition, button: PointerEventButton },
    PointerMoved { position: LogicalPosition },
    PointerScrolled { position: LogicalPosition, delta_x: f32, delta_y: f32 },
    PointerExited,

    // Touch events
    TouchPressed { touch_id: i32, position: LogicalPosition },
    TouchReleased { touch_id: i32, position: LogicalPosition },
    TouchMoved { touch_id: i32, position: LogicalPosition },

    // Keyboard events
    KeyPressed { text: SharedString },
    KeyPressRepeated { text: SharedString },
    KeyReleased { text: SharedString },

    // Window state events
    ScaleFactorChanged { scale_factor: f32 },
    Resized { size: LogicalSize },
    CloseRequested,
    WindowActiveChanged(bool),
}
```

**Dispatching events:**
```rust
// From platform backend to Slint
window.dispatch_event(WindowEvent::PointerPressed {
    position: LogicalPosition::new(100.0, 50.0),
    button: PointerEventButton::Left,
});
```

## WindowInner

Internal state management for windows:

```rust
pub struct WindowInner {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    component: RefCell<ItemTreeWeak>,
    strong_component_ref: RefCell<Option<ItemTreeRc>>,

    // Input state
    mouse_input_state: Cell<MouseInputState>,
    modifiers: Cell<InternalKeyboardModifierState>,
    click_state: ClickState,

    // Focus
    focus_item: RefCell<ItemWeak>,
    cursor_blinker: RefCell<PinWeak<TextCursorBlinker>>,

    // Property tracking
    pinned_fields: Pin<Box<WindowPinnedFields>>,  // scale_factor, active, etc.

    // Window state
    maximized: Cell<bool>,
    minimized: Cell<bool>,

    // Popups
    active_popups: RefCell<Vec<PopupWindow>>,
    next_popup_id: Cell<NonZeroU32>,

    // Callbacks
    close_requested: Callback<(), CloseRequestResponse>,
}
```

### Property Tracking

Windows use `PropertyTracker` to automatically request updates:

```rust
// Redraw tracker - requests redraw when any rendered property changes
struct WindowRedrawTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl PropertyDirtyHandler for WindowRedrawTracker {
    fn notify(self: Pin<&Self>) {
        if let Some(adapter) = self.window_adapter_weak.upgrade() {
            adapter.request_redraw();
        }
    }
}

// Properties tracker - notifies when window properties change
struct WindowPropertiesTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl PropertyDirtyHandler for WindowPropertiesTracker {
    fn notify(self: Pin<&Self>) {
        // Deferred update via timer
        Timer::single_shot(Default::default(), move || {
            // ... update_window_properties() ...
        });
    }
}
```

## Popup Management

### PopupWindow Structure

```rust
pub struct PopupWindow {
    pub popup_id: NonZeroU32,
    pub location: PopupWindowLocation,
    pub component: ItemTreeRc,
    pub close_policy: PopupClosePolicy,
    focus_item_in_parent: ItemWeak,
    pub parent_item: ItemWeak,
    is_menu: bool,
}

pub enum PopupWindowLocation {
    /// Separate top-level window
    TopLevel(Rc<dyn WindowAdapter>),
    /// Embedded in parent at position
    ChildWindow(LogicalPoint),
}

pub enum PopupClosePolicy {
    CloseOnClick,        // Close on any click
    CloseOnClickOutside, // Close only on click outside
    NoAutoClose,         // Manual close only
}
```

### Popup Placement

```rust
pub enum Placement {
    Fixed(LogicalRect),
}

/// Place popup within clip region (window/screen bounds)
pub fn place_popup(
    placement: Placement,
    clip_region: &Option<LogicalRect>,
) -> LogicalRect;
```

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

```rust
pub trait WinitCompatibleRenderer {
    fn render(&self, window: &Window) -> Result<(), PlatformError>;
    fn as_core_renderer(&self) -> &dyn Renderer;
    fn suspend(&self) -> Result<(), PlatformError>;
    fn resume(&self, ...) -> Result<Arc<winit::window::Window>, PlatformError>;
}
```

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

```rust
impl WindowProperties<'_> {
    /// Window title
    pub fn title(&self) -> SharedString;

    /// Background color/brush
    pub fn background(&self) -> Brush;

    /// Layout constraints (min, max, preferred size)
    pub fn layout_constraints(&self) -> LayoutConstraints;

    /// Fullscreen state
    pub fn is_fullscreen(&self) -> bool;

    /// Maximized state
    pub fn is_maximized(&self) -> bool;

    /// Minimized state
    pub fn is_minimized(&self) -> bool;
}

pub struct LayoutConstraints {
    pub min: Option<LogicalSize>,
    pub max: Option<LogicalSize>,
    pub preferred: LogicalSize,
}
```

## Input Method Support

For text input with IME:

```rust
pub enum InputMethodRequest {
    Enable(InputMethodProperties),
    Update(InputMethodProperties),
    Disable,
}

pub struct InputMethodProperties {
    pub text: SharedString,           // Surrounding text
    pub cursor_position: usize,       // Cursor byte offset
    pub anchor_position: Option<usize>, // Selection anchor
    pub preedit_text: SharedString,   // Pre-edit/composition text
    pub preedit_offset: usize,
    pub cursor_rect_origin: LogicalPosition,
    pub cursor_rect_size: LogicalSize,
    pub input_type: InputType,        // Text, Number, Password, etc.
}
```

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
// Conversion
let logical = physical_size.to_logical(scale_factor);
let physical = logical_size.to_physical(scale_factor);

// Window API uses both
fn position(&self) -> Option<PhysicalPosition>;  // Physical
fn set_size(&self, size: WindowSize);            // Can be either

pub enum WindowSize {
    Physical(PhysicalSize),
    Logical(LogicalSize),
}
```

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
let focus = window.focus_item();

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
