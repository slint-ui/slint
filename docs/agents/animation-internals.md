# Animation System Internals

> **When to load this document:** Working on `internal/core/animations.rs`,
> debugging animation timing issues, or optimizing animation performance.
> For general build commands and project structure, see `/AGENTS.md`.

## Animation Timing System

Slint animations use a **mocked time system** rather than real-time clocks. This provides:
- Deterministic animation behavior for testing
- Frame-rate independence
- Consistent behavior across platforms

The animation driver (`internal/core/animations.rs`) manages a global instant that advances each frame:

```
AnimationDriver
├── global_instant: Property<Instant>  // Current animation time
├── active_animations: bool            // Whether animations are running
└── update_animations(new_tick)        // Called per frame by the backend
```

**Key components:**

| Function/Type | Location | Purpose |
|---------------|----------|---------|
| `Instant` | `internal/core/animations.rs` | Milliseconds since animation driver started |
| `current_tick()` | `internal/core/animations.rs` | Get current animation time (registers dependency) |
| `animation_tick()` | `internal/core/animations.rs` | Same, but signals a frame is needed |
| `update_timers_and_animations()` | `internal/core/platform.rs` | Called by platform each frame |
| `EasingCurve` | `internal/core/items.rs` | Enum of easing curve types |

## Easing Curve Implementation

Easing curves are defined in the `EasingCurve` enum in `internal/core/items.rs`. The interpolation logic is in `internal/core/animations.rs`.

For `cubic-bezier(a, b, c, d)`, Slint uses a binary search algorithm to find the t parameter for a given x value, then evaluates the y component of the bezier curve.

Standard easings (`ease-in`, `ease-out`, `ease-in-out`, etc.) are pre-defined cubic bezier curves.

## Animation Performance

Each animated property:
1. Re-evaluates its binding every frame
2. Marks dependents dirty
3. Triggers re-rendering of affected items

**Efficient to animate** (no layout recalculation):
- `x`, `y` - Position
- `opacity` - Transparency
- `rotation-angle` - Rotation
- `background` - Colors/gradients

**Expensive to animate** (triggers layout):
- `width`, `height`
- `preferred-width`, `preferred-height`
- Any property that affects sibling positioning

## Debugging Animations

### Slow Motion

```sh
# Slow animations by factor of 4
SLINT_SLOW_ANIMATIONS=4 cargo run

# Slow by factor of 10 for detailed inspection
SLINT_SLOW_ANIMATIONS=10 cargo run
```

Useful for:
- Verifying easing curves
- Checking animation start/end states
- Debugging timing between multiple animations

### Checking Active Animations

```rust
// In application code
if window.has_active_animations() {
    // Animations are in progress
}
```

### Mock Time in Tests

For deterministic testing without real-time waits:

```rust
use slint_testing::mock_elapsed_time;

// Advance animation time by 100ms
mock_elapsed_time(100);

// Complete a 300ms animation
mock_elapsed_time(300);
```

This is implemented in `internal/core/tests/` and used throughout the test suite.

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/animations.rs` | Animation driver, timing, interpolation |
| `internal/core/items.rs` | `EasingCurve` enum definition |
| `internal/core/timers.rs` | Timer integration with animation system |
| `internal/core/platform.rs` | `update_timers_and_animations()` entry point |

## Common Modification Patterns

### Adding a New Easing Curve

1. Add variant to `EasingCurve` enum in `internal/core/items.rs`
2. Handle interpolation in `internal/core/animations.rs`
3. Add parsing support in `internal/compiler/` if new syntax needed
4. Add tests in `tests/cases/`

### Debugging Animation Glitches

1. Use `SLINT_SLOW_ANIMATIONS=10` to slow down
2. Check if issue is in timing (`animations.rs`) or rendering (`renderers/`)
3. Add `eprintln!` in `update_animations()` to trace tick values
4. Use screenshot tests to capture specific animation frames