# Animation System Internals

This document covers Slint's animation system architecture for developers working on the runtime.

For user-facing documentation on using animations, see:
- [Animation guide](astro/src/content/docs/guide/language/coding/animation.mdx)
- [Debugging techniques](astro/src/content/docs/guide/development/debugging_techniques.mdx)
- [Easing types reference](astro/src/content/docs/reference/primitive-types.mdx)

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
- `Instant` - Milliseconds since animation driver started
- `current_tick()` - Get current animation time (registers as dependency)
- `animation_tick()` - Same as above, but signals a frame is needed
- `update_timers_and_animations()` - Called by the platform each frame

## Easing Curve Implementation

Easing curves are defined in the `EasingCurve` enum in `internal/core/items.rs`. The interpolation logic is in `internal/core/animations.rs`.

For `cubic-bezier(a, b, c, d)`, Slint uses a binary search algorithm to find the t parameter for a given x value, then evaluates the y component of the bezier curve.
