# Item Tree & Component Model

> Note for AI coding assistants (agents):
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
| `internal/interpreter/instance.rs` | Runtime instance tree for the interpreter |
| `internal/interpreter/item_tree_vtable.rs` | `ItemTreeVTable` implementation for that instance |

## Tree Node Structure

Items are stored as a flat array with parent/child indices. An `ItemTreeNode` is either a
static `Item` — its parent index, the index and count of its children, its slot in the item
array, and whether it carries accessibility properties — or a `DynamicTree` placeholder holding
its parent index and the index passed to the `visit_dynamic` callback.
See `ItemTreeNode` in `internal/core/item_tree.rs`.

- Root item always at index 0
- Children stored contiguously
- `DynamicTree` nodes represent repeaters (dynamic content)

## Key Types

### ItemRc - Reference to an Item

A strong reference to the containing item tree plus the item's index within it.
See `ItemRc` in `internal/core/item_tree.rs`.

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
| Tree structure | Compile-time array | Flat array built at instantiation |
| Properties | Struct fields | Individually heap-allocated, indexed by LLR index |
| Bindings | Generated code | Runtime evaluation of the LLR expression |
| VTable | Generated per component | One static vtable shared by every instance |

**Interpreter key types** (`internal/interpreter/`):
- `Instance` (`instance.rs`) - the top-level item tree handed to i-slint-core, owning the flat
  node array and the tables that map each flat index back to its sub-component
- `SubComponentInstance` (`instance.rs`) - the runtime instance of one LLR sub-component: its
  properties, callbacks, items, nested sub-components and repeaters
- `ComponentDefinitionInner` / `ComponentInstanceInner` (`component.rs`) - what the public
  `ComponentDefinition` and `ComponentInstance` wrap

## Tree Traversal

### Traversal Order

`TraversalOrder` is `BackToFront` for rendering (background → foreground) and `FrontToBack` for
hit testing (foreground → background). See `internal/core/item_tree.rs`.

### Visitor Pattern

A visitor returns a `VisitChildrenResult`: either the `CONTINUE` constant, or `abort()` with the
item index and repeater index it stopped at. It is a packed `u64` rather than an enum because
that crosses the FFI boundary more easily. See `internal/core/item_tree.rs`.

### Traversal Uses

| Purpose | Order | Notes |
|---------|-------|-------|
| Rendering | BackToFront | Draw base layers first |
| Hit testing | FrontToBack | Top-most item wins |
| Tab focus | Forward | First child → next sibling |
| Shift+Tab | Backward | Previous sibling → parent |

## Focus Management

`internal/core/item_focus.rs` has three public functions, all taking an index into an
`ItemTreeNodeArray` and returning the next index or `None`:
`default_next_in_local_focus_chain()` (first child, else step out),
`default_previous_in_local_focus_chain()` (deepest last descendant of the previous sibling, else
the parent), and `step_out_of_node()`, which walks up until it finds a next sibling.

**Focus on ItemRc:**
- `next_focus_item()` - Tab key navigation
- `previous_focus_item()` - Shift+Tab navigation

## Component Instantiation

### Creating a Component

On the interpreter path, `ComponentDefinition::create_with_options()`
(`internal/interpreter/api.rs`) dispatches on a `WindowOptions` to one of the three constructors
on `ComponentDefinitionInner` in `internal/interpreter/component.rs`.

### Window Options

`WindowOptions` picks how the new instance gets its window: `CreateNewWindow`,
`UseExistingWindow` with an existing `WindowAdapterRc`, or `Embed` into a parent item tree at a
given index. See `internal/interpreter/api.rs`.

### Initialization Sequence

Still on the interpreter path (`Instance::new_with_options()` and `finalize_instance()` in
`internal/interpreter/instance.rs`):

1. Build the `SubComponentInstance` tree from the LLR
2. Build the flat `ItemTreeNode` array and the tables mapping each flat index back to the
   sub-component that owns it
3. Wrap it in a `VRc<ItemTreeVTable, Instance>` and back-link the weak references
4. Install the property bindings, and for a top-level instance attach the window adapter — before
   the init code, so `set_component()` can't clear a focus that `forward-focus` just set
5. Call `register_item_tree()`, which runs `Item::init()` on every native item and registers the
   tree with the window adapter
6. Run the component's `init_code`

### Cleanup

`unregister_item_tree()` (`internal/core/item_tree.rs`) walks the items once to:
- Free graphics resources
- Close dependent popups

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

`test-driver-interpreter` and `test-driver-rust` live in the separate `tests/` Cargo
workspace, and `gallery` lives in the separate `examples/` workspace, so these need an
explicit `--manifest-path` when run from the repository root:

```sh
# Run interpreter tests (exercises dynamic item tree)
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter

# Run Rust API tests
cargo test --manifest-path tests/Cargo.toml -p test-driver-rust

# Visual inspection
cargo run --manifest-path examples/Cargo.toml -p gallery
```
