# Item Tree & Component Model

> **When to load this document:** Working on `internal/core/item_tree.rs`,
> component instantiation, event handling, focus management, or understanding
> how compiled/interpreted Slint runs at runtime.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

The item tree is Slint's runtime representation of UI components:
- **Items** are individual UI elements (Rectangle, Text, TouchArea, etc.)
- **Item Trees** are hierarchical structures of items forming a component
- Both compiled and interpreted Slint use the same `ItemTreeVTable` interface

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/item_tree.rs` | ItemTree trait, ItemRc/ItemWeak, traversal |
| `internal/core/items.rs` | ItemVTable, built-in item definitions |
| `internal/core/item_focus.rs` | Focus chain traversal functions |
| `internal/core/item_rendering.rs` | ItemCache, rendering infrastructure |
| `internal/core/window.rs` | WindowInner, input handling |
| `internal/interpreter/dynamic_item_tree.rs` | Runtime ItemTree for interpreter |

## Tree Node Structure

Items are stored as a flat array with parent/child indices:

```rust
pub enum ItemTreeNode {
    Item {
        is_accessible: bool,      // Has accessibility info
        children_count: u32,      // Number of children
        children_index: u32,      // Index of first child
        parent_index: u32,        // Parent's index
        item_array_index: u32,    // Index in item storage
    },
    DynamicTree {
        index: u32,               // Repeater index
        parent_index: u32,
    },
}
```

- Root item always at index 0
- Children stored contiguously
- `DynamicTree` nodes represent repeaters (dynamic content)

## Key Types

### ItemRc - Reference to an Item

```rust
pub struct ItemRc {
    item_tree: VRc<ItemTreeVTable>,  // Containing tree
    index: u32,                       // Index within tree
}
```

**Navigation methods:**
- `parent_item()` - Get parent (with optional popup boundary)
- `first_child()` / `last_child()` - First/last child
- `next_sibling()` / `previous_sibling()` - Siblings
- `visit_descendants()` - Visit all descendants

### ItemWeak - Weak Reference

- Created via `ItemRc::downgrade()`
- Can become invalid if tree is destroyed
- Upgrade to `ItemRc` via `.upgrade()`

### ItemTreeVTable

The virtual function table all component trees implement:

| Function | Purpose |
|----------|---------|
| `visit_children_item` | Traverse children with visitor pattern |
| `get_item_ref` | Get item at index |
| `get_item_tree` | Get static tree structure |
| `parent_node` | Get parent item reference |
| `layout_info` | Get layout constraints |
| `item_geometry` | Get item position/size |
| `window_adapter` | Get/create window adapter |

## Compiled vs Interpreted

Both paths implement the same `ItemTreeVTable`:

| Aspect | Compiled | Interpreted |
|--------|----------|-------------|
| Tree structure | Compile-time array | `ItemTreeDescription` |
| Properties | Struct fields | Dynamic offsets |
| Bindings | Generated code | Runtime evaluation |
| VTable | Static | `dynamic_item_tree.rs` |

**Interpreter key types:**
- `ItemTreeDescription<'id>` - Component metadata
- `ItemTreeBox<'id>` - Instance container
- `InstanceRef<'a, 'id>` - Runtime instance access

## Tree Traversal

### Traversal Order

```rust
pub enum TraversalOrder {
    BackToFront,  // Rendering (background → foreground)
    FrontToBack,  // Hit testing (foreground → background)
}
```

### Visitor Pattern

```rust
pub struct VisitChildrenResult(u64);

impl VisitChildrenResult {
    pub const CONTINUE: Self;  // Keep traversing
    pub fn abort(index, repeater_index) -> Self;  // Stop here
}
```

### Traversal Uses

| Purpose | Order | Notes |
|---------|-------|-------|
| Rendering | BackToFront | Draw base layers first |
| Hit testing | FrontToBack | Top-most item wins |
| Tab focus | Forward | First child → next sibling |
| Shift+Tab | Backward | Previous sibling → parent |

## Focus Management

Focus traversal functions in `item_focus.rs`:

```rust
// Next item in tab order
pub fn default_next_in_local_focus_chain(index, item_tree) -> Option<u32>

// Previous item in tab order
pub fn default_previous_in_local_focus_chain(index, item_tree) -> Option<u32>

// Step out to sibling or parent's sibling
pub fn step_out_of_node(index, item_tree) -> Option<u32>
```

**Focus on ItemRc:**
- `next_focus_item()` - Tab key navigation
- `previous_focus_item()` - Shift+Tab navigation

## Component Instantiation

### Creating a Component

```rust
// Interpreter path
pub fn instantiate(
    description: Rc<ItemTreeDescription>,
    parent_ctx: Option<ErasedItemTreeBoxWeak>,
    root: Option<ErasedItemTreeBoxWeak>,
    window_options: Option<&WindowOptions>,
    globals: GlobalStorage,
) -> DynamicComponentVRc
```

### Window Options

```rust
pub enum WindowOptions {
    CreateNewWindow,                    // New window
    UseExistingWindow(WindowAdapterRc), // Attach to existing
    Embed { parent_item_tree, parent_item_tree_index }, // Sub-component
}
```

### Initialization Sequence

1. Allocate instance memory
2. Create `ItemTreeBox` wrapper
3. Initialize properties and bindings
4. Call `register_item_tree()` to init items
5. Register with window adapter

### Cleanup

```rust
pub fn unregister_item_tree(base, item_tree, item_array, window_adapter)
```
- Frees graphics resources
- Closes dependent popups

## Item VTable

Each item type implements `ItemVTable`:

| Function | Purpose |
|----------|---------|
| `init()` | Initialize after allocation |
| `layout_info()` | Return size constraints |
| `input_event()` | Handle mouse/touch |
| `input_event_filter_before_children()` | Filter events before children |
| `key_event()` | Handle keyboard |
| `focus_event()` | Handle focus changes |
| `render()` | Draw the item |
| `bounding_rect()` | Get bounds |

## Repeaters and Dynamic Content

Repeaters create dynamic subtrees:
- `DynamicTree` node in parent tree
- `get_subtree_range()` returns count of instances
- `get_subtree()` retrieves specific instance
- Each instance is a full `ItemTreeRc`

## Common Modification Patterns

### Adding a New Built-in Item

1. Define item struct in `internal/core/items.rs` or new file
2. Implement `Item` trait with required methods
3. Add to `ItemVTable` registration
4. Add to compiler's `builtins.slint`
5. Handle in renderers (`internal/renderers/*/`)

### Debugging Item Tree Issues

1. **Print tree structure**: Traverse with visitor, log indices
2. **Check parent/child**: Verify `children_index` and `parent_index`
3. **Focus issues**: Add logging in `item_focus.rs` functions
4. **Hit testing**: Log in `input_event_filter_before_children`

### Adding New Traversal Logic

1. Decide traversal order (BackToFront vs FrontToBack)
2. Implement visitor via `ItemVisitorVTable`
3. Call `visit_item_tree()` with your visitor
4. Handle `DynamicTree` nodes for repeaters

## Key Concepts for Agents

1. **Flat array with indices**: Tree stored as array, not nested structs
2. **Same interface for both paths**: Compiled and interpreted share `ItemTreeVTable`
3. **Visitor pattern**: All traversal uses visitors for flexibility
4. **Weak references for parents**: Avoids reference cycles
5. **DynamicTree for repeaters**: Repeaters are subtrees, not inline items
6. **Two-phase input**: Filter phase, then handle phase
7. **Index 0 is root**: Always start traversal from index 0

## Testing

```sh
# Run interpreter tests (exercises dynamic item tree)
cargo test -p test-driver-interpreter

# Run Rust API tests
cargo test -p test-driver-rust

# Visual inspection
cargo run -p gallery
```