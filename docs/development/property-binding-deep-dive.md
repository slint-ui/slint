# Property Binding & Reactivity Deep Dive

> Note for AI coding assistants (agents):
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
| `internal/core/properties/two_way_binding.rs` | Two-way binding via a shared common property |
| `internal/core/properties/ffi.rs` | FFI bindings for C++ interop |

## Core Data Structures

### Property<T>

A `Property<T>` is a `PropertyHandle` (the binding state and dependency list), the value itself
in an `UnsafeCell` — only safe to touch while the handle's lock flag is clear — and a
`PhantomPinned`. See `Property` in `internal/core/properties.rs`.

**Important**: Properties must be `Pin`ned because dependency nodes store raw pointers back to them. Moving a property would invalidate these pointers.

### PropertyHandle

`PropertyHandle` wraps a single `Cell<*mut ()>`. The pointer is always aligned, so its two least
significant bits are free for flags: `BINDING_BORROWED` (0b01), the lock flag that catches
recursion, and `BINDING_POINTER_TO_BINDING` (0b10), which says whether the pointer is a binding.
See `internal/core/properties.rs`.

The handle serves dual purpose:
- **With binding**: Points to a `BindingHolder` (bit 1 set)
- **Without binding**: Is the head of the dependency linked list

### BindingHolder

`BindingHolder<B>` wraps the binding callable `B` with: the head of the list of bindings that
depend on it, the nodes that link it into the dependency lists of the properties it reads, its
`BindingVTable`, a `dirty` flag, and a flag saying whether `B` is a `TwoWayBinding<T>`.
See `internal/core/properties.rs`.

### Dependency Tracking Structures

`DependencyListHead<T>` is the head of a doubly-linked list of dependents: a cell holding a
pointer to the first `DependencyNode<T>`. A `DependencyNode<T>` holds its `next` pointer, a `prev`
pointer that points at the *cell* pointing to itself (so the head and the nodes are unlinked the
same way), and the `T` — in practice a pointer to the `BindingHolder` that depends on us.
See `internal/core/properties.rs`.

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

Bindings don't evaluate immediately when marked dirty. `Property::get()` calls
`PropertyHandle::update()`, which does nothing unless the binding's `dirty` flag is set. When it
is, it clears the binding's dependency nodes, sets it as the current binding for the duration of
the call (`current_binding_storage::set()`, backed by the `CURRENT_BINDING` thread-local), invokes
the vtable's `evaluate`, and clears `dirty`. If `evaluate` returns `RemoveBinding`, the binding is
dropped and the value stays as it was left.
See `PropertyHandle::update` in `internal/core/properties.rs`.

## Two-Way Bindings

Two-way bindings link properties so changes to either propagate to both. A `TwoWayBinding<T>`
holds nothing but the shared backing property, a `Pin<Rc<Property<T>>>`.
See `internal/core/properties/two_way_binding.rs`.

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

For tracking dependencies outside of property bindings. A `PropertyTracker` is just a
`BindingHolder` around a dirty handler, which implements `PropertyDirtyHandler`. Its
`NEEDS_SET_DIRTY` const parameter says whether the tracker can also be dirtied from outside via
`set_dirty()`; when it can't — the default — a tracker with no tracked dependencies of its own
skips registering itself as a dependency of outer bindings, since nothing could ever dirty it.
See `internal/core/properties.rs`.

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

`AnimatedBindingCallable<T, A>` holds the underlying binding as a `PropertyHandle`, the
`Animating` / `NotAnimating` / `ShouldStart` state, the animation data, the closure `A` that
returns the animation parameters, and the tick captured by `mark_dirty`.
See `internal/core/properties/properties_animations.rs`.

**Animation flow:**
1. When the underlying binding changes, `mark_dirty` sets state to `ShouldStart`
2. On next `evaluate`, animation begins from current value to new binding value
3. Animation driver calls `update_animations()` to advance time
4. Each evaluation interpolates between from/to values
5. When finished, state returns to `NotAnimating`

## Constant Properties

Properties can be marked constant to optimize dependency tracking:

`set_constant()` points the property's dependency list head at the `CONSTANT_PROPERTY_SENTINEL`
static; `is_constant()` tests for it. See `internal/core/properties.rs`.

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

`PropertyHandle::access()` is how the binding is reached for evaluation: it asserts the lock flag
is clear (that assert is the "Recursion detected" panic), sets it, and clears it again from a
`scopeguard::defer!` so an unwinding binding still leaves the flag clean.
See `internal/core/properties.rs`.

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

Compile with `RUSTFLAGS='--cfg slint_debug_property'` to add a `debug_name` field to `Property`
and to `BindingHolder`. The "Recursion detected" panic then names the property involved. Note
that the field changes the struct layout, so such a build is not binary-compatible with C++.

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "Recursion detected" panic | Binding reads its own property | Break the cycle, use `get_untracked()` |
| Binding not updating | Dependency not registered | Ensure property read happens during binding evaluation |
| Memory leak | Circular Rc references | Use weak references in bindings |
| Stale value | Missing `mark_dirty` call | Ensure all value changes go through `set()` |

### Tracing Dependency Graph

```rust
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
