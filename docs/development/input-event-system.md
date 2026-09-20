# Input & Event System

> Note for AI coding assistants (agents):
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
| `internal/core/input.rs` | MouseEvent, event processing (re-exports `KeyEvent`/`KeyboardModifiers` from `internal/common/builtin_structs.rs`) |
| `internal/core/item_focus.rs` | Focus chain navigation |
| `internal/core/window.rs` | Window-level event dispatch |
| `internal/core/items.rs` | Item event handlers (input_event, etc.) |

## Mouse Events

### MouseEvent Enum

`MouseEvent` (`internal/core/input.rs`) is what an item's input handlers receive:

- `Pressed` and `Released`, with a position, a `PointerEventButton`, the click count, and a touch
  finger id (set for touch input, 0 for the mouse); `Moved`, with a position and finger id
- `Wheel` for the mouse wheel or a touchpad scroll: a position, an x and y delta, and a
  `TouchPhase`
- `DragMove` and `Drop`, each with the `DropEvent` and the allowed drag actions
- `PinchGesture` and `RotationGesture`, platform-recognized gestures (macOS/iOS trackpad, Qt),
  with a position, a delta and a `TouchPhase`
- `Exit`, when the pointer leaves the item

A backend dispatches a `BackendMouseEvent`: the same variants without `DragMove` and `Drop`,
so that `WindowEvent` stays `Send` and `Sync`.

### Click Counting

`ClickState` (`internal/core/input.rs`) tracks multi-clicks (double-click, triple-click) by
remembering the timestamp, count, position and button of the last press.

**Logic:**
- If press occurs within `click_interval` of previous press, at same position, with same button → increment `click_count`
- Otherwise reset to count 0
- `click_count` is included in Press/Release events

### Mouse Input State

`MouseInputState` (`internal/core/input.rs`) tracks the current state of mouse interaction:

- The stack of items containing the cursor (or the grab), each with the last result of its filter
  function, and whether the top item holds the mouse grab
- The passive observers that saw the last event without claiming it. They are kept out of the
  stack so it stays a single root-to-leaf path, and get a synthesized `MouseEvent::Exit` once they
  stop appearing
- The offset to apply to the first item of the stack, used when there is a popup
- The drag-and-drop state: the dragged data, the `DragArea` that started the drag (`None` for a
  native cross-window drag), and the `DropArea` that accepted the last `DragMove` — on release
  only that one gets the `Drop`, matching how OS drag-and-drop pipelines behave
- The delayed event and its timer (for `Flickable` touch handling), the items still owed an exit
  event, and the current mouse cursor

## Event Processing Flow

### Backend Entry Point

Every event a backend delivers goes through `Window::dispatch_event_with_result()`.
Events that the public `WindowEvent` variants can't express travel as `WindowEvent::Internal(InternalEvent)`,
a doc-hidden variant that carries the runtime's own `BackendMouseEvent`, `InternalKeyEvent` or touch point.
The dispatch reports them to the window event hook as the public event they correspond to,
or not at all when there is none (gestures, touch, input method composition).

`WindowInner::process_mouse_input()`, `process_key_input()` and `process_touch_input()` are crate-private,
so backends can't bypass that funnel and each event is observed exactly once.
Drag and drop is the exception: it uses `WindowInner::process_drag_event()`,
because the backend needs the negotiated `DragAction` back, which the public dispatch result can't express.
`WindowEvent` can't carry it anyway, being `Send` and `Sync` while the dragged payload is
reference counted.
That entry point takes a `BackendDragEvent`, so drag and drop is the only input that can travel it.

### Mouse Event Flow

```
┌─────────────────┐
│  Platform       │  (winit, Qt, etc.)
│  WindowEvent    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Window::       │  Single entry point for backend input
│  dispatch_event │  Notifies the window event hook
│  _with_result() │
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

Each item has two event handlers, both taking the `MouseEvent`, the window adapter, the item's
own `ItemRc`, and the mouse cursor to set: `input_event_filter_before_children()` runs before the
children and returns an `InputEventFilterResult`; `input_event()` runs after them, unless the
filter said otherwise, and returns an `InputEventResult`.

They are entries of `ItemVTable` in `internal/core/items.rs`, and the `Item` trait every item
implements is generated from it by `#[vtable]`. That trait has no defaults: an item that does not
care still has to write both out, returning `ForwardAndIgnore` and `EventIgnored` — `Empty` in the
same file is the shortest example.

### InputEventFilterResult

Controls how events are forwarded.
`InputEventFilterResult` (`internal/core/input.rs`) is one of:

- `ForwardEvent` - forward to the children, then call `input_event` on self
- `ForwardAndIgnore` - forward to the children, don't call `input_event` on self
- `ForwardAndInterceptGrab` - like `ForwardEvent`, but keep receiving events even if a child grabs
- `ForwardAndObserve` - like `ForwardAndIgnore`, but still get the `Exit` when the pointer leaves,
  even if a sibling handled the event in between
- `Intercept` - don't forward to the children; a child that already had the grab has it cancelled
  with an `Exit`
- `DelayForwarding(ms)` - forward after a delay, unless intercepted. Only for press events, and
  the event is sent early if a release arrives first (this is what `Flickable` uses)

### InputEventResult

Returned by `input_event`: `EventAccepted` (which may result in further events, e.g. accepting a
move leads to a later `Exit`), `EventIgnored`, `GrabMouse` to route all further mouse events to
this item, or `StartDrag`, which only a `DragArea` may return.
See `InputEventResult` in `internal/core/input.rs`.

## Mouse Grab

When an item returns `GrabMouse`:

1. All future mouse events go directly to that item
2. Events bypass the normal traversal
3. Grab continues until:
   - Item returns non-grab result
   - Mouse is released
   - An intercepting ancestor calls `Intercept`

`handle_mouse_grab()` (`internal/core/input.rs`) walks the item stack, translating the event into
each item's coordinates, and delivers it to the grabber. Items that asked for
`ForwardAndInterceptGrab` or `DelayForwarding` see it on the way down and may intercept, which
sends an `Exit` to everything below and drops it from the stack. It returns a `MouseGrabResult`:
the event that still needs normal hit-test dispatch (`None` when the grabber fully handled it),
and whether the grabber accepted the original event.

## Drag and Drop

### Starting a Drag

Only `DragArea` items can start drags:

```rust
// DragArea returns StartDrag from input_event
InputEventResult::StartDrag => {
    mouse_input_state.grabbed = false;
    mouse_input_state.drag_data = Some(DragData { ... });
}
```

### During Drag

Items receive `DragMove` events:

```rust
MouseEvent::DragMove { event: DropEvent { data, position, proposed_action }, allowed }
```

`data` is the `DataTransfer` payload the source set, `position` is in the item's local
coordinates, and `proposed_action` is the action negotiated from the current modifier state,
clamped to `allowed`.

Items return `EventAccepted` to indicate they can receive the drop.

### Drop

When mouse is released during drag:

```rust
MouseEvent::Drop { event: DropEvent { data, position, proposed_action }, allowed }
```

Only a `DropArea` that accepted the preceding `DragMove` receives it.

## Keyboard Events

### KeyEvent Structure

There are two: the public `KeyEvent` that reaches .slint callbacks, and the runtime's own
`InternalKeyEvent`.

`KeyEvent` (`internal/common/builtin_structs.rs`) has just the unicode `text` of the key, the
`KeyboardModifiers` active at the time, and a `repeat` flag that is true for auto-repeat presses
and always false for releases.

`KeyboardModifiers` (same file) is four bools: `alt`, `control`, `shift` and `meta`. On macOS
`control` is the Command key (⌘) and `meta` is the Control key; on Windows `meta` is the Windows
key.

`InternalKeyEvent` (`internal/core/input.rs`) wraps that public event and adds what the runtime
needs: the `KeyEventType`, the input-method composition fields (the replacement range, the
pre-edit text and its selection, the cursor and anchor positions) and, on Windows, the text
without modifiers — needed to tell Ctrl+Alt apart from AltGr.

`KeyEventType` is `KeyPressed`, `KeyReleased`, `UpdateComposition` (the input method updating the
pre-edit text) or `CommitComposition` (the composition's final result replacing it).

### Key Codes

Special keys are encoded as characters — the control characters where one exists (Backspace, Tab,
Return, Escape), Unicode private-use characters otherwise (the arrow keys, the function keys,
...). One table lists them all, along with each key's winit, Qt, xkb and muda name:
`for_each_keys!` in `internal/common/key_codes.rs`. The `key_codes` module of
`internal/core/input.rs` expands it into the `char` constants and the public `Key` enum, and each
backend expands the same table into its own key mapping.

### Keyboard Event Flow

```
Platform KeyEvent
       │
       ▼
Window::dispatch_event_with_result()
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

`InternalKeyEvent::shortcut()` recognizes the standard application shortcuts and
`text_shortcut()` the text-editing ones. Both are in `internal/core/input.rs`, and both are
platform-aware: Redo is Ctrl+Y on Windows and Ctrl+Shift+Z elsewhere, and the clipboard
shortcuts are left to the browser on wasm.

`StandardShortcut` is `Copy`, `Cut`, `Paste`, `SelectAll`, `Find`, `Save`, `Print`, `Undo`,
`Redo` and `Refresh`.

`TextShortcut` is `Move(TextCursorDirection)` plus the deletions: `DeleteForward`,
`DeleteBackward`, `DeleteWordForward`, `DeleteWordBackward` and `DeleteToStartOfLine`.

## Focus Management

### Focus State

The window tracks the currently focused item in `WindowInner::focus_item`, an `ItemWeak`.

### Setting Focus

`WindowInner::set_focus_item()` takes the item, a bool saying whether to focus it or clear the
focus, and the `FocusReason`. See `internal/core/window.rs`.

### FocusReason

`FocusReason` (`internal/common/enums.rs`) says what caused the change: `Programmatic` (a
`.focus()` or `.clear-focus()` call), `TabNavigation`, `PointerClick`, `PopupActivation`, or
`WindowActivation` when the window manager changed the active window.

### Focus Events

Items receive a `FocusEvent` — `FocusIn` or `FocusOut`, each carrying the `FocusReason` — and
answer with `FocusEventResult::FocusAccepted` or `FocusIgnored`; an ignored event is offered to
other items. See `internal/core/input.rs`.

### Focus Chain Navigation

Tab/Shift+Tab navigation traverses the item tree depth-first, children before siblings. Forward,
`default_next_in_local_focus_chain()` takes the first child if there is one, otherwise it steps
out to the next sibling or the nearest ancestor's next sibling. Backward,
`default_previous_in_local_focus_chain()` takes the deepest last descendant of the previous
sibling, or the parent when there is no previous sibling.
See `internal/core/item_focus.rs`, and [item-tree.md](item-tree.md#focus-management).

### Focus Delegation

Items can delegate focus via `forward-focus` property:

```slint,ignore
component MyInput {
    forward-focus: input;
    input := TextInput { }
}
```

## Text Cursor Blinker

For text input cursor animation. `TextCursorBlinker` (`internal/core/input.rs`) is a
`Property<bool>` plus the timer that toggles it. `set_binding()` binds a caller's property to that
visibility and starts the timer; `start()` and `stop()` control the blinking directly — `stop()`
is what runs when the window loses focus. `set_binding()` and `start()` both take the
`SlintContext` and the blink cycle duration; `stop()` takes neither.

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
    _cursor: &mut MouseCursorInner,
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
    _cursor: &mut MouseCursorInner,
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
let focus = WindowInner::from_pub(&window).focus_item.borrow().clone();
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
