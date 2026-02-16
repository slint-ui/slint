# Layout System Internals

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
- **FlexBoxLayout** - CSS Flexbox layout

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

```rust
pub struct LayoutInfo {
    pub min: Coord,           // Minimum size
    pub max: Coord,           // Maximum size
    pub min_percent: Coord,   // Minimum as % of parent
    pub max_percent: Coord,   // Maximum as % of parent
    pub preferred: Coord,     // Preferred size
    pub stretch: f32,         // Stretch factor (0.0 = don't stretch)
}
```

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
   → Shrink items proportionally (respecting min constraints)

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

Grid layouts solve independently for each axis:
1. **Organize**: Convert cell definitions to row/column assignments
2. **Solve horizontal**: Calculate column widths and x positions
3. **Solve vertical**: Calculate row heights and y positions

Cells with `colspan`/`rowspan` > 1 require iterative constraint distribution.

### FlexBox layout

FlexBox layout is solved in both axes simultaneously.
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
| `SolveFlexBoxLayout` | Compute positions and sizes for items in a flexbox layout |
| `ComputeLayoutInfo`  | Calculate combined constraints |
| `LayoutCacheAccess`  | Standard cache read |
| `GridRowCacheAccess` | Strided cache read (for nested repeaters in grids) |

## Key Data Structures

### Compiler-Side

```rust
// internal/compiler/layout.rs

pub struct GridLayout {
    pub elems: Vec<GridLayoutElement>,  // Cells
    pub geometry: LayoutGeometry,        // Padding, spacing, alignment
}

pub struct BoxLayout {
    pub orientation: Orientation,  // Horizontal or Vertical
    pub elems: Vec<LayoutItem>,
    pub geometry: LayoutGeometry,
}

pub struct LayoutConstraints {
    pub min_width: Option<NamedReference>,
    pub max_width: Option<NamedReference>,
    // ... other constraint properties as references
}
```

### Runtime

```rust
// internal/core/layout.rs

pub struct GridLayoutData {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: Padding,
    pub organized_data: GridLayoutOrganizedData,
}

pub struct BoxLayoutData<'a> {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: Padding,
    pub alignment: LayoutAlignment,
    pub cells: Slice<'a, LayoutItemInfo>,
}
```

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

Used by `HorizontalLayout`/`VerticalLayout`/`FlexBoxLayout` (via `LayoutCacheGenerator`).
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

### Strided cache (grid layouts with repeated rows)

Used by `GridLayout` (via `GridLayoutCacheGenerator`). When a repeater produces **Rows** —
i.e. each instance contributes **multiple children** at different column positions — accessing a
value requires **two indices**: the row instance index and the child offset within that row.
Each instance occupies a uniform stride of `step * entries_per_item` slots. The jump cell
stores both the data base offset and the stride.

**`repeater_steps`**: One entry per repeater — how many children each instance contributes.


**Example**: 1 repeater with 3 row instances, each having 2 children (step=2):

```
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
- `ri`: row instance index (0..count), runtime value from `$repeater_index`
- `stride`: `step * 2` — either a compile-time literal or read from `cache[jump_index + 1]` for rows containing inner repeaters
- `child_offset`: which child within the row (0, 2, 4, ...), compile-time known per child

### Multiple repeaters in one grid

Each repeater gets its own jump cell in the static region, and its own packed data block
in the dynamic region:

```
Example: 2 repeaters
  Repeater 0: 3 instances × 2 children (step=2), starts at cell 0
  Repeater 1: 2 instances × 3 children (step=3), starts at cell 6

repeater_indices = [0, 3, 6, 2]
repeater_steps   = [2, 3]

cache = [
  4., 4.,                                  // [0-1] rep 0 jump: data_base=4, stride=4 (step*2)
  16., 6.,                                 // [2-3] rep 1 jump: data_base=16, stride=6 (step*2)
  0., 50., 0., 50.,                        // [4-7]  rep 0 row 0 data
  50., 50., 50., 50.,                      // [8-11] rep 0 row 1 data
  100., 50., 100., 50.,                    // [12-15] rep 0 row 2 data
  150., 50., 150., 50., 150., 50.,         // [16-21] rep 1 row 0 data
  200., 50., 200., 50., 200., 50.,         // [22-27] rep 1 row 1 data
]
```

### Cache size formula

**Box layout (standard)**: `cells * 2 + repeater_indices.len()`

**Grid layout (strided)**: `(non_repeated_cells + num_repeaters) * 2 + sum(instance_count[i] * step[i] * 2)`

### How children read from the cache

During compile-time lowering (`lower_layout.rs`), each child element gets bindings like:

```
// Static child in a grid:
x: layout_cache_h[4]           // direct index, compile-time known

// Repeated child in box layout — standard cache:
x: layout_cache_h[cache[2] + $repeater_index * 2]

// Child of a repeated Row in grid — GridRow stride-based:
x: layout_cache_h[cache[jump_cell] + $repeater_index * stride + 0]

// width uses child_offset + 1:
width: layout_cache_h[cache[jump_cell] + $repeater_index * stride + 1]
```

These are represented as `Expression::LayoutCacheAccess` (standard) or
`Expression::GridRowCacheAccess` (grid repeated rows) in the expression tree, which
the code generators compile to the appropriate runtime access pattern.

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
2. **Independent axis solving**: Horizontal and vertical are solved separately (for horizontal, vertical and grid layouts)
3. **Constraint tightening**: Merging takes the most restrictive bounds
4. **Stretch factors**: Control how extra space is distributed (0 = don't grow)
5. **Cache indirection**: Enables repeaters without runtime structure changes
6. **Default geometry**: Elements default to 100% of parent unless content-sized

## Testing Layout Changes

```sh
# Run all layout-specific tests
cargo test -p test-driver-rust --test layout
cargo test -p test-driver-interpreter layout

# Run a specific test case, filtered by substring (don't prepend sh/bash, run_tests.sh is executable)
tests/run_tests.sh rust grid_conditional_row
tests/run_tests.sh interpreter grid_conditional_row
tests/run_tests.sh cpp grid_conditional_row

# Run all interpreter tests (fast)
cargo test -p test-driver-interpreter

# Visual verification (for humans)
cargo run -p gallery
```
