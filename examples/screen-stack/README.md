# Screen Stack Example

Demonstrates dynamic screen loading with **Model-Controller pattern** and **transition animations**.
Controllers are created when pushed onto the stack and destroyed when popped,
ensuring memory is only used for screens currently in the navigation stack.

## Features

- **Lazy loading**: Controllers created only when screen is pushed
- **Automatic cleanup**: Controllers destroyed when popped (memory freed)
- **Transition animations**: Fade, slide, or slide-fade transitions
- **Full navigation**: push, pop, replace, clear, pop-to-root
- **Centralized state**: AppStore for data that persists across screen lifecycles
- **Reusable templates**: TabView and ListScreen components

## Running

```bash
cargo run -p screen-stack
```

## Navigation API

### From Rust (ScreenContext)

```rust
ctx.push("screen-name");      // Push new screen onto stack
ctx.pop();                    // Pop current screen
ctx.replace("screen-name");   // Replace current screen (no back)
ctx.clear("screen-name");     // Clear stack, set new root
ctx.pop_to_root();            // Pop all screens, return to root
```

### From .slint (Navigation global)

```slint
Navigation.push("screen-name");
Navigation.pop();
Navigation.replace("screen-name");
Navigation.clear("screen-name");
Navigation.pop-to-root();
```

### Use Cases

| Method | Use Case |
|--------|----------|
| `push` | Normal navigation (can go back) |
| `pop` | Go back to previous screen |
| `replace` | Login → Home (prevent going back to login) |
| `clear` | Logout (reset entire navigation stack) |
| `pop_to_root` | Deep screen → Home (skip intermediate screens) |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  main.rs                                                    │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ AppStore (Centralized State)                          │  │
│  │   - selected_contact_id, selected_order_id, etc.      │  │
│  └───────────────────────────────────────────────────────┘  │
│                          │                                  │
│                          ▼                                  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ScreenManager                                         │  │
│  │   factories: HashMap<name, Fn() -> Controller>        │  │
│  │   stack: Vec<StackEntry>                              │  │
│  │   definitions: HashMap<name, ComponentDefinition>     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           │
         push("contacts")  │  pop()
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Stack Entry (exists only while on stack)                   │
│  ┌─────────────────────┐  ┌─────────────────────────────┐   │
│  │ ContactsScreen      │  │ ComponentInstance           │   │
│  │ (Controller)        │◄─┤ (UI from contacts.slint)    │   │
│  │                     │  │                             │   │
│  │ store: Rc<AppStore> │  │ Callbacks:                  │   │
│  │                     │  │   item-clicked, navigate    │   │
│  └─────────────────────┘  └─────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Transition Animations

Three transition types are available:

```slint
export enum TransitionType {
    slide,      // Slide left/right
    fade,       // Cross-fade (default)
    slide-fade, // Slide + fade combined
}

// Set in Navigation global:
Navigation.transition-type: TransitionType.fade;
```

## Screen Hierarchy

```
AbstractScreen          (base: title property)
    │
    ├── ScreenWithHeader    (header bar with back button)
    │       │
    │       ├── TabView         (horizontal tab bar + @children)
    │       │
    │       └── ListScreen      (scrollable list with model + item-clicked)
    │
    └── HomeScreen          (no header, custom layout)
```

## File Structure

```
examples/screen-stack/
├── Cargo.toml
├── build.rs
├── main.rs                     # App setup, screen registration
├── src/
│   ├── lib.rs
│   ├── screen.rs               # Screen trait, ScreenContext, helpers
│   ├── screen_manager.rs       # Stack management, lazy loading
│   ├── store.rs                # AppStore for centralized state
│   └── screens/
│       ├── mod.rs
│       ├── home.rs
│       ├── settings.rs
│       ├── contacts.rs         # Sets model from Rust
│       ├── contact_detail.rs
│       ├── orders.rs
│       ├── notifications_list.rs
│       ├── dashboard.rs        # TabView example
│       └── media.rs
└── ui/
    ├── main.slint              # App shell, Navigation global, animations
    ├── components/
    │   ├── abstract-screen.slint
    │   ├── screen-with-header.slint
    │   ├── tab-view.slint      # Tab bar template
    │   └── list-screen.slint   # List template (model property)
    └── screens/
        ├── home.slint
        ├── settings.slint
        ├── contacts.slint      # contacts <=> model binding
        ├── contact-detail.slint
        ├── orders.slint
        ├── dashboard.slint
        └── media.slint
```

## Screen Templates

### TabView

Horizontal tabs with content switching:

```slint
import { TabView } from "../components/tab-view.slint";

export component DashboardScreen inherits TabView {
    title: "Dashboard";
    tabs: ["Overview", "Stats", "Activity"];

    // Content per tab
    if root.current-tab == 0: Rectangle { /* Overview content */ }
    if root.current-tab == 1: Rectangle { /* Stats content */ }
    if root.current-tab == 2: Rectangle { /* Activity content */ }
}
```

### ListScreen

Scrollable list with model from Rust:

```slint
import { ListScreen, ListItem } from "../components/list-screen.slint";

export component ContactsScreen inherits ListScreen {
    title: "Contacts";
    empty-text: "No contacts yet";

    // Expose property for Rust access (inherited properties not settable via interpreter)
    in-out property <[ListItem]> contacts: [
        { id: "1", icon: "👤", title: "Alice", subtitle: "alice@example.com", has-arrow: true },
    ];

    // Bind to parent's model
    model: contacts;
}
```

## Controller Example

```rust
pub struct ContactsScreen {
    store: Rc<AppStore>,
    contacts: Vec<Contact>,
}

impl Screen for ContactsScreen {
    fn name(&self) -> &'static str { "contacts" }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Set model from Rust
        let items: Vec<Value> = self.contacts.iter().map(Self::to_list_item).collect();
        let model = ModelRc::new(VecModel::from(items));
        instance.set_property("contacts", Value::Model(model));

        // Handle item clicks
        let store = self.store.clone();
        let ctx = ctx.clone();
        instance.set_callback("item-clicked", move |args| {
            if let Some(Value::String(id)) = args.first() {
                store.set("selected_contact_id", id.to_string());
                ctx.push("contact-detail");
            }
            Value::Void
        });
    }
}
```

## Setting Model from Rust

The interpreter cannot set inherited properties directly. Use this pattern:

```slint
// In your-screen.slint
export component YourScreen inherits ListScreen {
    // 1. Declare exposed property with default (for preview)
    in-out property <[ListItem]> your-items: [ /* defaults */ ];

    // 2. Bind to parent's model
    model: your-items;
}
```

```rust
// In your_screen.rs
// Set the exposed property, not "model"
instance.set_property("your-items", Value::Model(model));
```

## Memory Management

```
>>> PUSH: home
[HomeScreen] Controller CREATED

>>> PUSH: contacts
[ContactsScreen] Controller CREATED

>>> PUSH: contact-detail
[ContactDetailScreen] Controller CREATED

>>> POP
>>> Stack depth now: 2
[ContactsScreen] on_loaded

>>> POP
>>> Stack depth now: 1
[ContactDetailScreen] Controller DESTROYED    ← Memory freed

>>> REPLACE: home -> settings
[HomeScreen] Controller DESTROYED
[SettingsScreen] Controller CREATED

>>> CLEAR: reset to home
[SettingsScreen] Controller DESTROYED
[HomeScreen] Controller CREATED
```

## Requirements

- Slint with experimental features enabled (`SLINT_ENABLE_EXPERIMENTAL_FEATURES=1`)
- Uses `ComponentContainer` and `component-factory` (experimental)
- Screen `.slint` files compiled at runtime (cached after first load)
