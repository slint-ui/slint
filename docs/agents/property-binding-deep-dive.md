# Property Binding & Reactivity Deep Dive

> **When to load this document:** Working on `internal/core/properties.rs`,
> debugging binding issues, implementing new property types, or understanding
> how Slint's reactive system works under the hood.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint's property system is the reactive foundation of the entire framework. Every UI element's state (position, color, text, visibility) is stored in properties. When properties change, dependent bindings automatically re-evaluate, keeping the UI in sync.

**Key characteristics:**
- **Lazy evaluation**: Bindings only re-evaluate when their value is actually read
- **Automatic dependency tracking**: Reading a property inside a binding automatically registers a dependency
- **Dirty marking**: Changes propagate instantly through the dependency graph, but evaluation is deferred

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/properties.rs` | Core Property<T>, bindings, dependency tracking |
| `internal/core/properties/change_tracker.rs` | ChangeTracker for property change callbacks |
| `internal/core/properties/properties_animations.rs` | Animated property values |
| `internal/core/properties/ffi.rs` | FFI bindings for C++ interop |

## Core Data Structures

### Property<T>

The main property type that holds a value and optional binding:

```rust
#[repr(C)]
pub struct Property<T> {
    handle: PropertyHandle,      // Binding state + dependency list
    value: UnsafeCell<T>,        // The actual value (interior mutability)
    pinned: PhantomPinned,       // Must be pinned for dependency tracking
}
```

**Important**: Properties must be `Pin`ned because dependency nodes store raw pointers back to them. Moving a property would invalidate these pointers.

### PropertyHandle

The handle manages binding state using bit flags in a single `usize`:

```rust
struct PropertyHandle {
    handle: Cell<usize>,
}

// Bit flags:
const BINDING_BORROWED: usize = 0b01;           // Lock flag (prevents recursion)
const BINDING_POINTER_TO_BINDING: usize = 0b10; // Has binding vs dependency list
```

The handle serves dual purpose:
- **With binding**: Points to a `BindingHolder` (bit 1 set)
- **Without binding**: Is the head of the dependency linked list

### BindingHolder

Wraps a binding callable with metadata:

```rust
#[repr(C)]
struct BindingHolder<B = ()> {
    dependencies: Cell<usize>,   // Head of dependents list (who depends on us)
    dep_nodes: Cell<...>,        // Nodes in other properties' dependency lists
    vtable: &'static BindingVTable,
    dirty: Cell<bool>,           // Needs re-evaluation?
    is_two_way_binding: bool,
    binding: B,                  // The actual binding callable
}
```

### Dependency Tracking Structures

```rust
// Head of a doubly-linked list of dependents
pub struct DependencyListHead<T>(Cell<*const DependencyNode<T>>);

// Node in the dependency list
pub struct DependencyNode<T> {
    next: Cell<*const DependencyNode<T>>,
    prev: Cell<*const Cell<*const DependencyNode<T>>>,  // Points to prev.next
    binding: T,  // Pointer to the BindingHolder that depends on us
}
```

## Dependency Tracking Flow

### How Dependencies Are Registered

When a binding evaluates and reads a property:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Property A     │     │  Binding B      │     │  Property C     │
│  (being read)   │     │  (evaluating)   │     │  (depends on A) │
└────────┬────────┘     └────────┬────────┘     └─────────────────┘
         │                       │
         │  1. B calls A.get()   │
         │<──────────────────────│
         │                       │
         │  2. A checks CURRENT_BINDING thread-local
         │     (finds B is currently evaluating)
         │                       │
         │  3. A adds B to its dependency list
         │     (B now listed as dependent on A)
         │                       │
         │  4. B stores a DependencyNode pointing to A
         │     (so B can unregister when re-evaluated)
         │                       │
```

**Code path:**
1. `Property::get()` calls `handle.update()` then `register_as_dependency_to_current_binding()`
2. `CURRENT_BINDING` thread-local contains the currently evaluating binding
3. The binding's `DependencyNode` is added to the property's `DependencyListHead`

### How Changes Propagate

When a property value changes:

```
┌─────────────────┐           ┌─────────────────┐
│  Property A     │──────────>│  Binding B      │
│  value changed  │  mark     │  dirty=true     │
└────────┬────────┘  dirty    └────────┬────────┘
         │                             │
         │                             │ (B has dependents too)
         │                             ▼
         │                    ┌─────────────────┐
         │                    │  Binding C      │
         │                    │  dirty=true     │
         │                    └─────────────────┘
```

**Code path:**
1. `Property::set()` calls `handle.mark_dirty()`
2. `mark_dependencies_dirty()` iterates the dependency list
3. Each dependent binding's `dirty` flag is set to `true`
4. The vtable's `mark_dirty` callback is invoked (for animations, etc.)
5. Recursively marks dependents of dependents

### Lazy Evaluation

Bindings don't evaluate immediately when marked dirty. Instead:

```rust
// In Property::get()
unsafe { self.handle.update(self.value.get()) };  // Only evaluates if dirty

// In PropertyHandle::update()
if binding.dirty.get() {
    // Clear old dependencies
    binding.dep_nodes.set(Default::default());

    // Evaluate with CURRENT_BINDING set to this binding
    CURRENT_BINDING.set(Some(binding), || {
        (binding.vtable.evaluate)(...)
    });

    binding.dirty.set(false);
}
```

## Two-Way Bindings

Two-way bindings link properties so changes to either propagate to both:

```rust
struct TwoWayBinding<T> {
    common_property: Pin<Rc<Property<T>>>,  // Shared backing property
}
```

**How it works:**
1. Both properties get a `TwoWayBinding` that points to a shared "common property"
2. Reading either property reads from the common property
3. Setting either property sets the common property (which notifies both)
4. The `intercept_set` callback redirects writes to the common property

```
┌──────────┐     ┌─────────────────┐     ┌──────────┐
│ Property │────>│ Common Property │<────│ Property │
│    A     │     │   (shared)      │     │    B     │
└──────────┘     └─────────────────┘     └──────────┘
     │                   │                    │
     └───────────────────┴────────────────────┘
              All reads/writes go here
```

## PropertyTracker

For tracking dependencies outside of property bindings:

```rust
pub struct PropertyTracker<DirtyHandler = ()> {
    holder: BindingHolder<DirtyHandler>,
}
```

**Usage:**
```rust
let tracker = Box::pin(PropertyTracker::default());

// Evaluate and track dependencies
let value = tracker.as_ref().evaluate(|| {
    prop_a.as_ref().get() + prop_b.as_ref().get()
});

// Check if any dependency changed
if tracker.is_dirty() {
    // Re-evaluate...
}
```

**With dirty handler:**
```rust
let tracker = PropertyTracker::new_with_dirty_handler(|| {
    // Called immediately when any dependency changes
    schedule_repaint();
});
```

## ChangeTracker

For running callbacks when property values actually change:

```rust
let change = ChangeTracker::default();
change.init(
    data,                           // User data passed to callbacks
    |data| property.get(),          // Eval function (reads property)
    |data, new_value| { ... },      // Notify function (called on change)
);

// Later, process all pending changes:
ChangeTracker::run_change_handlers();
```

**Key difference from PropertyTracker:**
- `PropertyTracker`: Notified when dependencies become dirty
- `ChangeTracker`: Notified when the evaluated value actually changes

## Animation Integration

Animated properties use special bindings:

```rust
pub struct AnimatedBindingCallable<T, A> {
    original_binding: PropertyHandle,  // The underlying binding
    state: Cell<AnimatedBindingState>, // Animating/NotAnimating/ShouldStart
    animation_data: RefCell<PropertyValueAnimationData<T>>,
    compute_animation_details: A,      // Returns animation parameters
}
```

**Animation flow:**
1. When the underlying binding changes, `mark_dirty` sets state to `ShouldStart`
2. On next `evaluate`, animation begins from current value to new binding value
3. Animation driver calls `update_animations()` to advance time
4. Each evaluation interpolates between from/to values
5. When finished, state returns to `NotAnimating`

## Constant Properties

Properties can be marked constant to optimize dependency tracking:

```rust
static CONSTANT_PROPERTY_SENTINEL: u32 = 0;

// A property is constant if its dependency list head points to the sentinel
pub fn set_constant(&self) {
    // ... sets dependency head to point to CONSTANT_PROPERTY_SENTINEL
}
```

When reading a constant property, no dependency is registered (optimization).

## Pin and Unsafe Patterns

### Why Pin?

Properties must be pinned because:
1. `DependencyNode` stores raw pointers to `DependencyListHead`
2. `DependencyListHead` stores raw pointers to `DependencyNode`
3. Moving either would invalidate these pointers

### Key Unsafe Invariants

1. **Lock flag**: The `BINDING_BORROWED` flag must be set before accessing `value` and cleared after
2. **Dependency list integrity**: `prev` and `next` pointers must remain valid while nodes exist
3. **CURRENT_BINDING**: Must be restored after binding evaluation
4. **VTable safety**: `BindingHolder<B>` must only be cast via its own vtable

### Safe Accessors

```rust
// Safe way to access binding - handles lock flag
fn access<R>(&self, f: impl FnOnce(Option<Pin<&mut BindingHolder>>) -> R) -> R {
    assert!(!self.lock_flag(), "Recursion detected");
    self.set_lock_flag(true);
    scopeguard::defer! { self.set_lock_flag(false); }
    // ... access binding ...
}
```

## Common Patterns

### Creating a Reactive Component

```rust
#[derive(Default)]
struct MyComponent {
    input: Property<i32>,
    output: Property<i32>,  // Will be bound to input * 2
}

let comp = Rc::pin(MyComponent::default());
let weak = Rc::downgrade(&comp);

comp.output.set_binding(move || {
    let comp = weak.upgrade().unwrap();
    Pin::new(&comp.input).get() * 2
});
```

### Detecting Property Changes

```rust
// Using PropertyTracker
let tracker = Box::pin(PropertyTracker::new_with_dirty_handler(|| {
    println!("Something changed!");
}));
tracker.as_ref().evaluate(|| {
    a.get() + b.get()
});

// Using ChangeTracker
let change = ChangeTracker::default();
change.init((), |_| property.get(), |_, val| println!("New value: {}", val));
```

### Two-Way Binding Between Properties

```rust
let prop1 = Rc::pin(Property::new(42));
let prop2 = Rc::pin(Property::new(0));

Property::link_two_way(prop1.as_ref(), prop2.as_ref());
// Now prop1 and prop2 are synchronized
```

## Debugging Tips

### Enable Debug Names

Compile with `RUSTFLAGS='--cfg slint_debug_property'` to enable property debug names:

```rust
#[cfg(slint_debug_property)]
pub debug_name: RefCell<String>,
```

This helps identify which property is involved in recursion errors.

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "Recursion detected" panic | Binding reads its own property | Break the cycle, use `get_untracked()` |
| Binding not updating | Dependency not registered | Ensure property read happens during binding evaluation |
| Memory leak | Circular Rc references | Use weak references in bindings |
| Stale value | Missing `mark_dirty` call | Ensure all value changes go through `set()` |

### Tracing Dependency Graph

```rust
// Check if property has binding
prop.handle.access(|b| b.is_some())

// Check if property is dirty
prop.is_dirty()

// Check if property is constant
prop.is_constant()
```

## Testing

```sh
# Run property system tests
cargo test -p i-slint-core properties

# Run with debug names enabled
RUSTFLAGS='--cfg slint_debug_property' cargo test -p i-slint-core properties

# Run animation tests
cargo test -p i-slint-core animation_tests
```

## Performance Considerations

1. **Binding allocation**: Each binding allocates a `BindingHolder` on the heap
2. **Dependency list traversal**: `mark_dirty` traverses all dependents recursively
3. **Lazy evaluation**: Avoids unnecessary computation but can cause latency spikes
4. **Constant properties**: Skip dependency registration entirely

For hot paths, consider:
- Using `get_untracked()` when dependency tracking isn't needed
- Marking properties constant when they won't change
- Batching property changes to reduce dirty propagation
