# Ink Particle Artwork

It draws tinted glass, liquid surfaces, and reflective bubbles through four RGB565 Slint images.
Particles compute exact fractional coverage with row recurrences and restore only their previous footprints.
Background restoration reads the empty and liquid spans directly from their assets.
The neutral renderer caches seven surface rows; Pico tilt uses eight fractionally sampled rows per column.
Two image buffers per vial avoid copying a buffer still referenced by Slint.

## Vial Holds

At either supported clock, holding a Pico vial adds 60 percent to its existing weighted bubble population.
Every extra bubble uses the maximum radius of five pixels; normal bubbles retain their original size distribution.
Extra bubbles form at the bottom with staggered births, then rise through the liquid.
Releasing stops new births; existing extra bubbles continue rising until they reach the liquid surface.
Pressing again keeps live bubbles and fills only vacant slots, so repeated presses cannot grow the population without limit.
Normal particle counts and sizes remain unchanged.
See the [Pico qualification](../../../examples/mcu-board-support/pico2_touch_lcd_2_8/results/report.md) for measurement commands and the stock-speed policy.

## Pico Tilt and Orientation

The QMI8658 accelerometer and gyroscope share the touch controller's I2C bus.
A complementary filter predicts gravity from gyro readings, corrects it toward acceleration, and learns stationary gyro bias.
Linear acceleration reduces the correction weight.
Near-flat poses smoothly return to screen-up rather than retaining an arbitrary heading.

The surface counter-rotates with device roll, limited to 30 degrees either way.
Its center stays at the selected fill height.
A sheared, antialiased ellipse with fractional vertical sampling preserves the curved meniscus through zero tilt.
Surface colors are cached when the normal or fill changes.
Each retained buffer restores its old bubble footprints and the union of the old and new surface bands.
Unchanged glass and liquid spans outside that union stay intact.
Bubble force follows the full fused up vector, independently of the surface limit.
Persistent bubble positions slide along the capsule-shaped glass boundary.

Holding the device in the opposite landscape orientation for 400 ms flips the whole UI with the panel's hardware addressing mode.
The detector requires a gravity projection beyond 0.6 in either vertical direction; flat and sideways poses keep the current orientation.
Touch coordinates and the gravity vector rebase with the flip.
Transfer ordering follows physical scan gates in both orientations.

Profiling adds `mode ink-tilt` and `mode ink-tilt-boost`, which replay consistent accelerometer and gyro samples through the same fusion and orientation code.
`tilt X Y` overrides the output with a Q10 unit vector; `tilt live` restores sensor input.
`rotation 0`, `rotation 1`, and `rotation auto` allow repeatable native-orientation checks.

## Native Pico Assets

`ink-native-assets` borrows static backgrounds and masks directly from flash for 48 × 140 pixel vials.
These are the native vial dimensions in the 320 × 240 printer window.
Other sizes use the same procedural renderer without resizing the assets.
The data contains two little-endian RGB565 backgrounds and one eight-bit coverage mask per vial, totaling 134,400 bytes.
Glass rims are already baked into the backgrounds; separate rim masks are not needed after construction.
The flash-backed path avoids 134,400 bytes of heap allocations at this size.
The identical meniscus and opaque-row tables are also shared in flash, saving another 4,928 bytes across the four vials.

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
On Pico, core 1 composes the Yellow and Cyan vials in bounded batches while core 0 composes Red and Black.
Core 0 allocates tilt storage and reserves damage storage before submission.
Both cores prepare and paint their own vials, borrowing disjoint plain state and pixel buffers.
Core 0 joins the worker before publishing Slint images.
The worker handles only plain renderer state and pixels; Slint objects remain on core 0.
Each batch restores eight rows, or two while the surface is tilted, or stamps one particle, allowing display DMA and USB tasks to run between batches.
The worker sleeps when no frame is submitted.
The [two-core measurements](../../../examples/mcu-board-support/pico2_touch_lcd_2_8/results/stock-150/core1-ink/report.md) separate composition savings from Slint raster time.

Leaving Ink releases its particle and image buffers.
Kiosk releases them before constructing the next page to avoid retaining both working sets.
The particle tests also cover cached restoration, resize, density, rising speed, glass preservation, and hold/release behavior.

The profiling command `mode ink` opens the scene when the ink feature is enabled.
`mode ink-boost` holds all four vials for stress testing; `mode ink-touch` replays individual pointer holds and releases.
`mode auto`, `mode pan`, and `mode pan-fast` return to the original queue workload.
Use `mode manual` for normal interaction.
Native framebuffer captures remain available while Ink runs.
Panel-memory checks compare the framebuffer directly with controller GRAM without allocating a second framebuffer.
