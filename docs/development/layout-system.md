# Layout System Internals

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/core/layout.rs`,
> `internal/compiler/passes/lower_layout.rs`, debugging sizing/positioning issues,
> or implementing new layout features.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint's layout system has two phases:
1. **Compile-time**: Layout elements are lowered to constraint expressions and cache structures
2. **Runtime**: Constraints are evaluated and positions/sizes are calculated

Layout types:
- **HorizontalLayout / VerticalLayout** - Linear box layouts
- **GridLayout** - 2D grid with row/column positioning, spans
- **Dialog** - Special grid with platform-specific button ordering
- **FlexboxLayout** - CSS Flexbox layout

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/layout.rs` | Runtime layout solving algorithms |
| `internal/compiler/layout.rs` | Compiler-side layout data structures |
| `internal/compiler/passes/lower_layout.rs` | Lowers layout elements to expressions |
| `internal/compiler/passes/default_geometry.rs` | Sets default width/height (runs after layout lowering) |
| `internal/compiler/llr/lower_layout_expression.rs` | Converts layout expressions to LLR |

## Constraint System

### LayoutInfo (Runtime)

The constraints for one item along one axis: a min and a max, the same two again as a percentage
of the parent, a preferred size, and a stretch factor (0.0 means don't stretch). Sizes are
`Coord`, the stretch is an `f32`. See `LayoutInfo` in `internal/core/layout.rs`.

### Constraint Merging

When constraints combine (e.g., nested layouts):
- **min**: Take the larger (tightest constraint)
- **max**: Take the smaller (tightest constraint)
- **preferred**: Take the larger
- **stretch**: Take the smaller

### Constraint Properties

Elements can specify these properties:
- `min-width`, `min-height`
- `max-width`, `max-height`
- `preferred-width`, `preferred-height`
- `horizontal-stretch`, `vertical-stretch`

## Layout Solving Algorithm

Both grid and box layouts use the same core algorithm in `layout_items()`:

```
1. Set initial sizes to preferred values
2. Calculate total size needed

3. If total > available space:
   → Shrink items weighted by their stretch factors, respecting min constraints.
     With no stretch factor set anywhere, each item gives up the same number of
     pixels, so a small item runs out long before a large one. Items that reach
     their min are frozen and the rest is re-split over the others.

4. If total < available space:
   → Grow items proportionally based on stretch factors
   → Items with stretch=0 stay at preferred size

5. Assign positions sequentially with spacing
```

### Box Layout Alignment

When items fit without shrinking, alignment determines positioning:

| Alignment | Behavior |
|-----------|----------|
| `Stretch` | Grow items to fill space (default) |
| `Start` | Pack at beginning |
| `Center` | Pack in center |
| `End` | Pack at end |
| `SpaceBetween` | Equal gaps between items |
| `SpaceAround` | Equal gaps around items |
| `SpaceEvenly` | Equal gaps including edges |

### Grid Layout

Grid layouts solve each axis on its own, except that the vertical pass measures
height-for-width cells at the width the horizontal pass solved:
1. **Organize**: Convert cell definitions to row/column assignments
2. **Solve horizontal**: Calculate column widths and x positions
3. **Solve vertical**: Calculate row heights and y positions

Cells with `colspan`/`rowspan` > 1 require iterative constraint distribution.

### Flexbox layout

Flexbox layout is solved in both axes simultaneously.
The layouting algorithm is provided by the `taffy` crate, which implements the CSS flexbox algorithm.

## Compile-Time Lowering

The `lower_layout.rs` pass transforms layout elements:

```
GridLayout element
    ↓
lower_grid_layout()
    ↓
Creates synthetic properties:
  - layout-organized-data (cell organization)
  - layout-cache-h (horizontal positions/sizes)
  - layout-cache-v (vertical positions/sizes)
  - layoutinfo-h, layoutinfo-v (constraints)
    ↓
Child x/y/width/height bound to cache access expressions
```

### Key Expressions Generated

| Expression | Purpose |
|------------|---------|
| `OrganizeGridLayout` | Compute cell row/column assignments |
| `SolveBoxLayout`     | Compute positions and sizes for items in a box layout |
| `SolveGridLayout`    | Compute positions and sizes for items in a grid layout |
| `SolveFlexboxLayout` | Compute positions and sizes for items in a flexbox layout |
| `ComputeLayoutInfo`  | Calculate combined constraints |
| `LayoutCacheAccess`  | Read position/size from cache |
| `GridRepeaterCacheAccess` | Two-level indirection cache read (for repeaters in grids) |

## Key Data Structures

### Compiler-Side

All in `internal/compiler/layout.rs`:

- `GridLayout` - the cells plus the `LayoutGeometry` (padding, spacing, alignment). It also
  carries the button roles when the grid is really a `Dialog`, and whether any row/column
  expression uses `auto`.
- `BoxLayout` - the orientation, the items and the same `LayoutGeometry`, plus the
  `cross-axis-alignment` property if one was set.
- `LayoutConstraints` - one `Option<NamedReference>` per `min-`/`max-`/`preferred-` width and
  height and per stretch, the two fixed-size flags, and a `LayoutConstraintLocality` with one bool
  per named reference recording whether it was set on the element itself rather than inherited
  from a base component. Inherited ones are already baked into the element's `layoutinfo-*`, so a
  parent that measured the cell through its layout-info must not re-apply them.

### Runtime

Both in `internal/core/layout.rs`:

- `GridLayoutData` - the available size, the spacing and padding, and the
  `GridLayoutOrganizedData` produced by `organize_grid_layout()`.
- `BoxLayoutData` - the available size, the spacing and padding, the `LayoutAlignment`, and a
  borrowed slice of `LayoutItemInfo`, one per cell.

## Layout Cache Formats

The layout cache is a flat `SharedVector<Coord>` (i.e. `SharedVector<f32>`) storing solved
positions and sizes for all children of a layout. Each child occupies 2 slots: `[pos, size]`
(e.g. `[x, width]` for horizontal, `[y, height]` for vertical). There are separate caches
for horizontal and vertical axes.

### Static-only layout (no repeaters)

When all children are known at compile time, the cache is a simple flat array.

```
cache = [pos0, size0, pos1, size1, ..., posN, sizeN]
```

Access: `cache[index]` where `index = child_idx * 2` for pos, `child_idx * 2 + 1` for size.

### Standard cache (box layouts)

Used by `HorizontalLayout`/`VerticalLayout`/`FlexboxLayout` (via `LayoutCacheGenerator`).
Static children occupy a fixed slot; each repeater instance contributes exactly one cell (one pos +
one size). When repeaters are present, their instances are stored in a contiguous block at
the end of the cache, with a jump cell in the static region pointing to the start of that
block.

**`repeater_indices`**: Pairs of `(start_cell_index, instance_count)` — one pair per repeater.

**Example**: 1 fixed cell, then a repeater with 3 instances

```
repeater_indices = [1, 3]  // repeater starts at cell 1, has 3 instances

cache = [
  0., 50.,         // fixed cell: pos=0, size=50
  4., 5.,          // jump cell: points to offset 4 (first dynamic slot)
  80., 50.,        // repeated instance 0
  160., 50.,       // repeated instance 1
  240., 50.,       // repeated instance 2
]
```

**Access**: `cache[cache[jump_index] + repeater_index * entries_per_item]`

- `jump_index`: the cache index of the jump cell (compile-time known)
- `repeater_index`: which instance (0..count), runtime value
- `entries_per_item`: 2 for the coordinate cache (pos + size), compile-time known

### Two-level indirection cache (grid layouts with repeaters)

Used by `GridLayout` (via `GridLayoutCacheGenerator`) for any repeater, whether single-item or multi-child.
Like the standard cache, it uses jump cells for indirection, but with a key difference: the stride is **variable and dynamic**.

For box layouts, the stride is always fixed at `entries_per_item` (2 for coordinates). For grid layouts with repeaters,
the stride is `step * entries_per_item`, where `step` is the number of children per instance. The stride can be:
- **Compile-time constant**: When all repeater children are static
- **Runtime value**: When a repeater instance contains nested repeaters, retrieved from the jump cell itself

This enables grids to handle both single-item repeaters (step=1) and multi-child repeaters (step=N) with potentially nested repeaters inside.

**`repeater_steps`**: A vector with one entry per repeater — how many children each instance contributes.

**Example**: 1 repeater with 3 row instances, each having 2 children (step=2):

```
slint! {
    GridLayout {
        for _ in 3: Row {
            Rectangle {}
            Rectangle {}
        }
    }
};

repeater_indices = [0, 3]   // starts at cell 0, 3 instances
repeater_steps   = [2]      // 2 children per instance

cache = [
  2., 4.,                    // [0-1] jump cell: data_base=2, stride=4 (step*2)
  0., 50., 0., 50.,          // [2-5] row 0 data: child0=(pos=0,size=50), child1=(pos=0,size=50)
  50., 50., 50., 50.,        // [6-9] row 1 data
  100., 50., 100., 50.,      // [10-13] row 2 data
]
```

If rows have different numbers of children (jagged), the stride is based on the maximum number
of children across all rows, and shorter rows are padded to match that stride.

**Access**: `cache[cache[jump_index] + ri * stride + child_offset]`

- `jump_index`: compile-time known (index of the jump cell, always `jump_cell_pos * 2`)
- `ri`: repeater instance index (0..count), runtime value from `$repeater_index`
- `stride`: `step * 2` — either a compile-time literal (for static repeater children) or read from `cache[jump_index + 1]` (for rows containing nested repeaters)
- `child_offset`: which child within the rows (0, 2, 4, ...), compile-time known per child

### How children read from the cache

During compile-time lowering (`lower_layout.rs`), each child element gets bindings like:

```
// Static child in a grid:
x: layout_cache_h[4]           // direct index, compile-time known
width: layout_cache_h[5]

// Repeated child in box layout — standard cache (LayoutCacheAccess):
x: layout_cache_h[cache[2] + $repeater_index * 2]
width: layout_cache_h[cache[2] + $repeater_index * 2 + 1]

// Repeated element in grid layout (even single-item) — two-level indirection cache (GridRepeaterCacheAccess):
// For single-item: step=1, stride=2 (step * entries_per_item)
// For multiple children per repeater: step=N, stride=N*2
x: layout_cache_h[cache[jump_cell] + $repeater_index * stride + child_offset]
width: layout_cache_h[cache[jump_cell] + $repeater_index * stride + child_offset + 1]
```

These are represented as `Expression::LayoutCacheAccess` (standard, for box layouts and static items in grids) or
`Expression::GridRepeaterCacheAccess` (grid repeaters with any repeater structure) in the expression tree, which
the code generators compile to the appropriate runtime access pattern.

## Measuring repeated cells

Where no cross size is passed in — the GridLayout solve, a `VerticalLayout` main
pass — a static height-for-width cell (a word-wrapped `Text`, say) sizes itself:
its `width` is bound to the layout cache, so the `Text` reads the width the
layout gave it while the layout-info is computed (`text_layout_info` in
`i-slint-core` treats a cross constraint below zero as "use the current width").
Other passes hand static cells an explicit constraint.

A repeated cell cannot read its own width that way: the layout asks the whole
instance for its layout-info, and the instance goes through
`layoutinfo-v-with-constraint` rather than reading `self.width` (see
`synthesize_layoutinfo_v_with_constraint`), so it is measured at a fixed cross
size — the instance's preferred width for the vertical info, an unbounded height
for the horizontal one. The layout therefore passes the real size in, through
the accessors on `RepeatedItemTree`:

| Accessor | Backed by | Supplied by |
|---|---|---|
| `layout_item_info_at_cross_width(w)` | `SubComponent::layout_info_v_at_cross_width_for_repeated` | any vertical pass at a known width: `VerticalLayout` main pass, `HorizontalLayout` ortho pass, GridLayout vertical pass |
| `layout_item_info_at_cross_height(h)` | `SubComponent::layout_info_h_at_cross_height_for_repeated` | any horizontal pass at a known height: `HorizontalLayout` main pass, `VerticalLayout` ortho pass |
| `flexbox_layout_item_info_at_cross_width(w)` / `_height(h)` | the same two expressions | FlexboxLayout solve |

Which accessor is used depends on the orientation being computed, not on the box
layout's own direction: a `VerticalLayout` calls
`layout_item_info_at_cross_width` from its main pass and
`layout_item_info_at_cross_height` from its ortho pass, a `HorizontalLayout` the
other way around. GridLayout appears in the first row only. It solves horizontal
first, so measuring a cell at a solved height would make the horizontal solve
read the vertical cache, which the end of this section explains it must not.

The `SubComponent` fields are in `internal/compiler/llr/item_tree.rs`; the
generators emit the accessors in `internal/compiler/generator/rust.rs` and
`generator/cpp.rs`, and the interpreter mirrors them in
`internal/interpreter/eval_layout.rs` and `instance.rs`.

Where the size comes from differs per layout kind:

- **Box layout, main pass** forwards one size for all cells (the layout's cross
  content size), in `Expression::WithLayoutItemInfo::repeated_cross_size`.
- **Box layout, ortho pass** has no single size to forward:
  `Expression::BoxLayoutInfoOrthoWithMeasure` solves the main axis first, then
  measures each instance at its *own* solved main size, as a
  `BoxMeasureCell::Repeated`. `repeated_cross_size` is `None` here.
- **GridLayout** has one width per column, so it reads each cell's own slot out
  of `layout-cache-h`: `LayoutRepeatedElement::cross_width`
  (`internal/compiler/layout.rs`) is the cell's own `width` binding with the
  repeater index replaced by the `GRID_MEASURE_REPEATER_INDEX_LOCAL` local,
  which the generated loop binds to the instance index. A *repeated* child of a
  repeated `Row` uses `SubComponent::grid_row_child_cross_width` instead,
  addressed by the child's flattened index (`GRID_MEASURE_CHILD_INDEX_LOCAL`).
  One expression serves every such child, and
  `RowChildTemplateInfo::Repeated::measure_at_cross_width` records which ones it
  applies to. A static child of a `Row` keeps its plain, unconstrained
  layout-info.
- **FlexboxLayout** passes a size twice: the container's cross width up front,
  in `Expression::WithFlexboxLayoutItemInfo::repeated_cross_width` (column flex
  only), and then per cell from taffy's measure callback
  (`SolveFlexboxLayoutWithMeasure`, `FlexboxLayoutInfoCrossAxisWithMeasure`),
  which re-measures at the size taffy actually assigns.

Reading the horizontal cache from the vertical pass is only sound while the
horizontal solve does not read back into the vertical cache. A *repeated*
width-for-height cell breaks that: the grid measures it through its plain
`layout_info_h`, which pulls the instance's own height, and that height comes
from the grid's vertical cache. `mark_grid_h_solve_reads_v_cache` sets
`GridLayoutCell::h_solve_reads_v_cache` on every cell of such a grid, and the
vertical pass then falls back to the instance's plain layout-info instead of
closing a binding loop. Static cells are safe: on the grid solve
`cell_layout_info` passes no cross size, so a cell with
`layoutinfo-h-with-constraint` is measured at an unbounded height and one
without it never reads its own height.

## Common Modification Patterns

### Adding a New Layout Property

1. Add property to builtin layout element in `internal/compiler/builtins.slint`
2. Handle in `LayoutGeometry` or `LayoutConstraints` in `internal/compiler/layout.rs`
3. Update `lower_layout.rs` to extract and use the property
4. Update runtime structs in `internal/core/layout.rs` if needed
5. Add tests in `tests/cases/layout/`

### Debugging Layout Issues

1. **Check constraint propagation**: Add `eprintln!` in `LayoutInfo::merge()`
2. **Check solving**: Add logging in `layout_items()` to see shrink/grow steps
3. **Verify cache access**: Check `LayoutCacheAccess` indices in generated code
4. **Use inspector**: Run with Slint inspector to see element bounds

### Adding a New Alignment Mode

1. Add variant to `LayoutAlignment` enum in `internal/core/layout.rs`
2. Handle in `solve_box_layout()` alignment switch
3. Add parsing in compiler if new syntax needed
4. Add tests for the new alignment

## Key Concepts for Agents

1. **Two-phase architecture**: Compile-time creates structure, runtime evaluates values
2. **Independent axis solving**: Horizontal and vertical are solved separately (for horizontal, vertical and grid layouts), except where the vertical pass measures a height-for-width cell at the solved width
3. **Constraint tightening**: Merging takes the most restrictive bounds
4. **Stretch factors**: Control how extra space is distributed (0 = don't grow)
5. **Cache indirection**: Enables repeaters without runtime structure changes
6. **Default geometry**: Elements default to 100% of parent unless content-sized

## Testing Layout Changes

`test-driver-rust` and `test-driver-interpreter` live in the separate `tests/` Cargo
workspace, and `gallery` lives in the separate `examples/` workspace, so these need an
explicit `--manifest-path` when run from the repository root (`tests/run_tests.sh`
already handles this for you):

```sh
# Run all layout-specific tests
cargo test --manifest-path tests/Cargo.toml -p test-driver-rust --test layout
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter layout

# Run a specific test case, filtered by substring (don't prepend sh/bash, run_tests.sh is executable)
tests/run_tests.sh rust grid_conditional_row
tests/run_tests.sh interpreter grid_conditional_row
tests/run_tests.sh cpp grid_conditional_row

# Run all interpreter tests (fast)
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter

# Visual verification (for humans)
cargo run --manifest-path examples/Cargo.toml -p gallery
```
