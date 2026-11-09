# Child Bindings

The experimental `child-bindings` feature connects a container to the components supplied through its default `@children` slot.
Use it when each child needs parent state, an index, and callbacks to coordinate with its siblings.
Segmented buttons and radio groups can declare their own contracts through the same mechanism.

Enable `interfaces` and `child-bindings` in the project's experimental features or `SLINT_ENABLE_EXPERIMENTAL_FEATURES`.
Standard widgets enable these features internally.

```slint
interface Entry {
    in property <int> position;
    in property <bool> selected;
    callback activated();
}

component EntryGroup {
    in-out property <int> current-index: -1;
    out property <int> count: entries.count;

    child-bindings <Entry> entries {
        position: entries.index;
        selected: root.current-index == entries.index;
        activated => { root.current-index = entries.index; }
    }

    HorizontalLayout { @children }
}
```

The interface describes the required properties and callbacks.
Children satisfy it structurally and retain their component type, visual children, slots, and behavior.
The compiler copies the block's bindings and handlers onto each supplied child.
It resolves references to the container separately for every instance, including references to the container's internal elements.

Inside the block, `self` and `entries` refer to the current child.
`root` refers to the container.
`entries.index` is the child's zero-based position.
`entries.count` is the number of managed children and can also be read elsewhere in the container.
Both metadata properties are read-only; child interfaces cannot declare `index` or `count`.
The collection identifier cannot be used as a visual element or to access individual children outside the block.

The block accepts property bindings, two-way bindings, callback connections, and property change handlers.
A supplied child cannot bind a property assigned by the block or provide another handler for a callback connected by the block.
Property change handlers are combined.
Properties and callbacks omitted from the block remain available for application bindings.
Named slot content is excluded from the managed collection.

## Current Restrictions

- Declare one block on the component root and provide a default `@children` placeholder.
- Supply static children or a single `for` loop.
  The loop's model determines the count, and each delegate receives its current model index.
- Conditional children and mixtures of static children and loops are unsupported.
- Derived components inherit the contract and cannot replace it.
  If a derived component supplies managed children, its instances cannot add more managed children.
- A wrapper cannot forward its own `@children` into an inner component with child bindings.
  Inherit the container instead.

These restrictions keep collection indexes unambiguous without a runtime child registry.
Supporting mixed dynamic collections would require extending this generic mechanism.

## Compiler Integration

The parser represents the declaration as `ChildBindings`.
Object tree construction creates an interface-backed template and a container property for its count.
The template participates in expression resolution and type checking.

`passes/lower_child_bindings.rs` validates children, specializes the template for each instance, and removes the template before component inlining.
It reuses interface validation and inherited-expression forwarding.
The resulting properties and callbacks use the existing runtime and code generators.
No control-specific registration or runtime primitive is required.

