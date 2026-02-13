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
| `LayoutCacheAccess`  | Read position/size from cache |

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

## Repeaters in Layouts

Repeaters (dynamic item lists) in layouts use indirection:
- `repeater_indices`: Maps repeater to starting cell index
- Layout cache uses jump tables for repeated items
- Allows compile-time cache structure with runtime item counts

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

# Run a specific test case (filtered by substring)
tests/run_tests.sh rust grid_conditional_row
tests/run_tests.sh interpreter grid_conditional_row
tests/run_tests.sh cpp grid_conditional_row

# Run all interpreter tests (fast)
cargo test -p test-driver-interpreter

# Visual verification (for humans)
cargo run -p gallery
```
