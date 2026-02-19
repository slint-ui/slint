# Input & Event System

> **When to load this document:** Working on `internal/core/input.rs`,
> `internal/core/item_focus.rs`, `internal/core/window.rs` event handling,
> mouse/keyboard/touch processing, or focus management.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint's input system handles mouse, touch, keyboard events and focus management. Events flow from the platform through the window to items in the item tree, with support for:

- **Mouse/touch events**: Press, release, move, wheel, drag-drop
- **Keyboard events**: Key press/release, text input, IME composition
- **Focus management**: Tab navigation, programmatic focus, focus delegation
- **Event filtering**: Items can intercept, delay, or forward events

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/input.rs` | MouseEvent, KeyEvent, event processing |
| `internal/core/item_focus.rs` | Focus chain navigation |
| `internal/core/window.rs` | Window-level event dispatch |
| `internal/core/items.rs` | Item event handlers (input_event, etc.) |

## Mouse Events

### MouseEvent Enum

```rust
pub enum MouseEvent {
    /// Mouse/finger pressed
    Pressed {
        position: LogicalPoint,
        button: PointerEventButton,
        click_count: u8,
        is_touch: bool,
    },

    /// Mouse/finger released
    Released {
        position: LogicalPoint,
        button: PointerEventButton,
        click_count: u8,
        is_touch: bool,
    },

    /// Pointer moved
    Moved { position: LogicalPoint, is_touch: bool },

    /// Mouse wheel
    Wheel { position: LogicalPoint, delta_x: Coord, delta_y: Coord },

    /// Drag operation in progress over item
    DragMove(DropEvent),

    /// Drop occurred on item
    Drop(DropEvent),

    /// Mouse exited the item
    Exit,
}
```

### Click Counting

The `ClickState` tracks multi-clicks (double-click, triple-click):

```rust
pub struct ClickState {
    click_count_time_stamp: Cell<Option<Instant>>,
    click_count: Cell<u8>,
    click_position: Cell<LogicalPoint>,
    click_button: Cell<PointerEventButton>,
}
```

**Logic:**
- If press occurs within `click_interval` of previous press, at same position, with same button → increment `click_count`
- Otherwise reset to count 0
- `click_count` is included in Press/Release events

### Mouse Input State

Tracks the current state of mouse interaction:

```rust
pub struct MouseInputState {
    /// Stack of items under cursor, with their filter results
    item_stack: Vec<(ItemWeak, InputEventFilterResult)>,

    /// Offset for popup positioning
    pub(crate) offset: LogicalPoint,

    /// True if an item has grabbed the mouse
    grabbed: bool,

    /// Active drag-drop data
    pub(crate) drag_data: Option<DropEvent>,

    /// Delayed event (for Flickable touch handling)
    delayed: Option<(Timer, MouseEvent)>,

    /// Items pending exit events
    delayed_exit_items: Vec<ItemWeak>,
}
```

## Event Processing Flow

### Mouse Event Flow

```
┌─────────────────┐
│  Platform       │  (winit, Qt, etc.)
│  WindowEvent    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  WindowInner::  │  Click counting, modifier tracking
│  process_mouse_ │
│  input()        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  handle_mouse_  │  Check if item has grab
│  grab()         │  If so, send directly to grabber
└────────┬────────┘
         │ (if no grab)
         ▼
┌─────────────────┐
│  process_mouse_ │  Traverse item tree
│  input()        │  front-to-back
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  send_mouse_    │  For each item:
│  event_to_item()│  1. filter_before_children
│                 │  2. recurse to children
│                 │  3. input_event
└─────────────────┘
```

### Item Event Handlers

Each item has two event handlers:

```rust
// Called before children process the event
fn input_event_filter_before_children(
    &self,
    event: &MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
    self_rc: &ItemRc,
) -> InputEventFilterResult;

// Called after children (unless filtered)
fn input_event(
    &self,
    event: &MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
    self_rc: &ItemRc,
) -> InputEventResult;
```

### InputEventFilterResult

Controls how events are forwarded:

```rust
pub enum InputEventFilterResult {
    /// Forward to children, then call input_event on self
    ForwardEvent,

    /// Forward to children, don't call input_event on self
    ForwardAndIgnore,

    /// Forward, but keep receiving events even if child grabs
    ForwardAndInterceptGrab,

    /// Don't forward to children, handle here
    Intercept,

    /// Delay forwarding (for touch scrolling detection)
    DelayForwarding(u64),  // milliseconds
}
```

### InputEventResult

Returned by `input_event`:

```rust
pub enum InputEventResult {
    /// Event was handled
    EventAccepted,

    /// Event was not handled, continue propagation
    EventIgnored,

    /// Grab all future mouse events until release
    GrabMouse,

    /// Start drag-drop operation (DragArea only)
    StartDrag,
}
```

## Mouse Grab

When an item returns `GrabMouse`:

1. All future mouse events go directly to that item
2. Events bypass the normal traversal
3. Grab continues until:
   - Item returns non-grab result
   - Mouse is released
   - An intercepting ancestor calls `Intercept`

```rust
// In handle_mouse_grab()
if mouse_input_state.grabbed {
    // Send event directly to grabber
    let grabber = mouse_input_state.top_item().unwrap();
    let result = grabber.input_event(&event, ...);

    match result {
        InputEventResult::GrabMouse => None,  // Keep grab
        _ => {
            mouse_input_state.grabbed = false;
            Some(MouseEvent::Moved { ... })  // Resume normal processing
        }
    }
}
```

## Drag and Drop

### Starting a Drag

Only `DragArea` items can start drags:

```rust
// DragArea returns StartDrag from input_event
InputEventResult::StartDrag => {
    mouse_input_state.grabbed = false;
    mouse_input_state.drag_data = Some(DropEvent {
        mime_type: drag_area.mime_type(),
        data: drag_area.data(),
        position: Default::default(),
    });
}
```

### During Drag

Items receive `DragMove` events:

```rust
MouseEvent::DragMove(DropEvent { mime_type, data, position })
```

Items return `EventAccepted` to indicate they can receive the drop.

### Drop

When mouse is released during drag:

```rust
MouseEvent::Drop(DropEvent { mime_type, data, position })
```

## Keyboard Events

### KeyEvent Structure

```rust
pub struct KeyEvent {
    pub text: SharedString,           // Character or key code
    pub modifiers: KeyboardModifiers, // Alt, Ctrl, Shift, Meta
    pub event_type: KeyEventType,
    // ... IME composition fields
}

pub enum KeyEventType {
    KeyPressed,
    KeyReleased,
    UpdateComposition,  // IME pre-edit
    CommitComposition,  // IME finalized
}

pub struct KeyboardModifiers {
    pub alt: bool,
    pub control: bool,
    pub meta: bool,
    pub shift: bool,
}
```

### Key Codes

Special keys are encoded as Unicode private-use characters:

```rust
pub mod key_codes {
    pub const Backspace: char = '\u{0008}';
    pub const Tab: char = '\u{0009}';
    pub const Return: char = '\u{000D}';
    pub const Escape: char = '\u{001B}';
    pub const LeftArrow: char = '\u{F702}';
    pub const RightArrow: char = '\u{F703}';
    pub const UpArrow: char = '\u{F700}';
    pub const DownArrow: char = '\u{F701}';
    // ... more in key_codes module
}
```

### Keyboard Event Flow

```
Platform KeyEvent
       │
       ▼
WindowInner::process_key_input()
       │
       ├── Update modifier state
       │
       ├── If popup active → send to popup
       │
       └── Send to focus item
              │
              ├── Item handles → KeyEventResult::EventAccepted
              │
              └── Item ignores → bubble up to parent
                     │
                     └── Continue until handled or root
```

### Shortcuts

```rust
impl KeyEvent {
    /// Check for standard shortcuts (Ctrl+C, etc.)
    pub fn shortcut(&self) -> Option<StandardShortcut>;

    /// Check for text editing shortcuts
    pub fn text_shortcut(&self) -> Option<TextShortcut>;
}

pub enum StandardShortcut {
    Copy, Cut, Paste, SelectAll, Find, Save, Print, Undo, Redo, Refresh,
}

pub enum TextShortcut {
    Move(TextCursorDirection),
    DeleteForward, DeleteBackward,
    DeleteWordForward, DeleteWordBackward,
    DeleteToStartOfLine,
}
```

## Focus Management

### Focus State

The window tracks the currently focused item:

```rust
// In WindowInner
focus_item: RefCell<crate::item_tree::ItemWeak>,
```

### Setting Focus

```rust
pub fn set_focus_item(
    &self,
    new_focus_item: &ItemRc,
    set_focus: bool,       // true = focus, false = clear focus
    reason: FocusReason,
)
```

### FocusReason

```rust
pub enum FocusReason {
    /// Focus changed via click
    PointerAction,
    /// Focus changed via Tab key
    TabNavigation,
    /// Focus changed via code (forward-focus, etc.)
    Other,
}
```

### Focus Events

Items receive focus events:

```rust
pub enum FocusEvent {
    FocusIn(FocusReason),
    FocusOut(FocusReason),
}

pub enum FocusEventResult {
    FocusAccepted,
    FocusIgnored,
}
```

### Focus Chain Navigation

Tab/Shift+Tab navigation traverses the item tree:

```rust
// Forward: depth-first, children before siblings
fn default_next_in_local_focus_chain(index: u32, item_tree: &ItemTreeNodeArray) -> Option<u32> {
    // First try first child
    if let Some(child) = item_tree.first_child(index) {
        return Some(child);
    }
    // Then try next sibling, or parent's next sibling
    step_out_of_node(index, item_tree)
}

// Backward: reverse of forward
fn default_previous_in_local_focus_chain(index: u32, item_tree: &ItemTreeNodeArray) -> Option<u32> {
    // Try previous sibling's deepest descendant
    if let Some(previous) = item_tree.previous_sibling(index) {
        Some(step_into_node(item_tree, previous))
    } else {
        // Or parent
        item_tree.parent(index)
    }
}
```

### Focus Delegation

Items can delegate focus via `forward-focus` property:

```slint,ignore
component MyInput {
    forward-focus: input;
    input := TextInput { }
}
```

## Text Cursor Blinker

For text input cursor animation:

```rust
pub struct TextCursorBlinker {
    cursor_visible: Property<bool>,
    cursor_blink_timer: Timer,
}

impl TextCursorBlinker {
    /// Create binding that toggles visibility
    pub fn set_binding(
        instance: Pin<Rc<TextCursorBlinker>>,
        prop: &Property<bool>,
        cycle_duration: Duration,
    );

    /// Start blinking
    pub fn start(self: &Pin<Rc<Self>>, cycle_duration: Duration);

    /// Stop blinking (e.g., window loses focus)
    pub fn stop(&self);
}
```

## Delayed Event Handling

For touch interfaces, `Flickable` delays events to distinguish scroll from tap:

```rust
InputEventFilterResult::DelayForwarding(duration_ms)
```

**Flow:**
1. Flickable returns `DelayForwarding(150)` on touch press
2. Timer starts, event is stored
3. If release comes before timeout → forward original press, then release
4. If movement detected → Flickable handles as scroll, original target never sees press

## Common Patterns

### Implementing Custom Input Handling

```rust
fn input_event(
    self: Pin<&Self>,
    event: &MouseEvent,
    _window_adapter: &Rc<dyn WindowAdapter>,
    self_rc: &ItemRc,
) -> InputEventResult {
    match event {
        MouseEvent::Pressed { button: PointerEventButton::Left, .. } => {
            // Handle press
            InputEventResult::GrabMouse  // Capture further events
        }
        MouseEvent::Released { .. } => {
            // Handle release
            InputEventResult::EventAccepted
        }
        MouseEvent::Moved { position, .. } => {
            // Handle move (only received if grabbed)
            InputEventResult::GrabMouse
        }
        _ => InputEventResult::EventIgnored,
    }
}
```

### Intercepting Child Events

```rust
fn input_event_filter_before_children(
    self: Pin<&Self>,
    event: &MouseEvent,
    _window_adapter: &Rc<dyn WindowAdapter>,
    _self_rc: &ItemRc,
) -> InputEventFilterResult {
    if self.should_intercept(event) {
        InputEventFilterResult::Intercept
    } else {
        InputEventFilterResult::ForwardEvent
    }
}
```

### Handling Keyboard Focus

```rust
fn focus_event(
    self: Pin<&Self>,
    event: &FocusEvent,
    _window_adapter: &Rc<dyn WindowAdapter>,
    _self_rc: &ItemRc,
) -> FocusEventResult {
    match event {
        FocusEvent::FocusIn(_) => {
            // Start cursor blink, etc.
            FocusEventResult::FocusAccepted
        }
        FocusEvent::FocusOut(_) => {
            // Stop cursor blink, etc.
            FocusEventResult::FocusAccepted
        }
    }
}
```

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Item not receiving events | Not in event path | Check item geometry, clips_children |
| Click not working | Event being grabbed | Check for GrabMouse returns |
| Focus not moving | forward-focus loop | Check focus delegation chain |
| Double-click not detected | Click interval too short | Check platform click_interval |
| Touch scroll not working | DelayForwarding not used | Check Flickable setup |

### Tracing Events

```rust
// Add logging in input_event
fn input_event(...) -> InputEventResult {
    eprintln!("input_event: {:?} on {:?}", event, self_rc.index());
    // ...
}
```

### Checking Focus

```rust
// Get current focus item
let focus = window.focus_item();
if let Some(item) = focus.upgrade() {
    println!("Focused: {:?}", item.index());
}
```

## Testing

```sh
# Run input handling tests
cargo test -p i-slint-core input

# Run focus tests
cargo test -p i-slint-core item_focus

# Run with specific test
cargo test -p i-slint-core test_focus_chain
```
