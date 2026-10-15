# Ink Particle Artwork

The vial renderer composes glass, liquid, and particles into four Slint images.
It draws tinted glass, liquid surfaces, and reflective bubbles through four RGB565 Slint images.
Particles compute exact fractional coverage with row recurrences and restore only their previous footprints.
Background restoration reads the empty and liquid spans directly from their assets and caches only seven liquid-surface rows.
At native dimensions, four vials save 51,072 bytes versus retaining four composed backgrounds.
Two image buffers per vial avoid copying a buffer still referenced by Slint.

## Vial Holds

At either supported clock, holding a Pico vial adds 60 percent to its existing weighted bubble population.
Every extra bubble uses the maximum radius of five pixels; normal bubbles retain their original size distribution.
Extra bubbles form at the bottom with staggered births, then rise through the liquid.
Releasing stops new births; existing extra bubbles continue rising until they reach the liquid surface.
Pressing again keeps live bubbles and fills only vacant slots, so repeated presses cannot grow the population without limit.
Normal particle counts and trajectories remain unchanged.
See the [Pico qualification](../../../examples/mcu-board-support/pico2_touch_lcd_2_8/results/report.md) for measurement commands and the stock-speed policy.

## Native Pico Assets

`ink-native-assets` borrows static backgrounds and masks directly from flash for 48 × 140 pixel vials.
These are the native vial dimensions in the 320 × 240 printer window.
Other sizes use the same procedural renderer without resizing the assets.
The data contains two little-endian RGB565 backgrounds and one eight-bit coverage mask per vial, totaling 134,400 bytes.
Glass rims are already baked into the backgrounds; separate rim masks are not needed after construction.
The flash-backed path avoids 134,400 bytes of heap allocations at this size.

Regenerate the assets from the same renderer:

```sh
rustc --edition=2024 -O demos/printerdemo_mcu/tools/generate-ink-assets.rs -o /tmp/generate-ink-assets
/tmp/generate-ink-assets demos/printerdemo_mcu/ink_particles/assets
```

The `borrowed_background_spans_preserve_full_composition_at_every_level` test checks all 1,025 fill levels against the full-background reference.
The `native_assets_preserve_procedural_frames` test compares generated and procedural frames across all four vials, five fill levels, and four animation phases.
Keep this test enabled when changing artwork or regenerating the data.

## Timing and Memory

With `pico2`, animation advances once per measured physical panel tick.
The timer polls the published tick; it does not assume a 60 Hz wall-clock interval.
Leaving Ink releases its particle and image buffers.
Kiosk releases them before constructing the next page to avoid retaining both working sets.
The particle tests also cover cached restoration, resize, density, rising speed, glass preservation, and hold/release behavior.

The profiling command `mode ink` opens the scene when the ink feature is enabled.
`mode ink-boost` holds all four vials for stress testing; `mode ink-touch` replays individual pointer holds and releases.
`mode auto`, `mode pan`, and `mode pan-fast` return to the original queue workload.
Use `mode manual` for normal interaction.
Native framebuffer captures remain available while Ink runs.
Panel-memory checks compare the framebuffer directly with controller GRAM without allocating a second framebuffer.
