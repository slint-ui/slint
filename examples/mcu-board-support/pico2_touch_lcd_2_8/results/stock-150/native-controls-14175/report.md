# Pico Native Controls Regression Check

## Source and Method

The Pico printer changes are rebased onto [PR #14175](https://github.com/slint-ui/slint/pull/14175), head `eb347e2df9229bb28d4fa861623cfd3083537b05`.
The previous optimized branch remains intact at `9c73986225cc95e6256692a319823edd80bc2edc`.
The integration branch is `nigel/pico2-printer-native-controls`.

Both firmware versions were rebuilt with the same Rust compiler and tested on Pico serial `BFC13272EF67B96D`.
Both use 150 MHz CPU, 75 MHz quad flash, 48 MHz display SPI, ThinLTO, one code-generation unit, and Cortex-M33 code generation.
The cadence image enables coarse profiling without the nested renderer instrumentation.
Each scripted workload measurement covers ten seconds after a two-second settling period.
Queue and Settings use whole-pixel scrolling at native 320 × 240 resolution.

This comparison includes the upstream commits between the old base and the PR head.
It measures the complete integration and does not isolate the native-control PR from the other upstream changes.

[Before measurements](before.json) and [firmware metadata](firmware.json) identify the measured source and binaries.

## Matched Before and After

The final comparison repeats the baseline workload order after a verified BOOTSEL-to-application restart.
The restart was confirmed by reset run and window counters before selecting a workload.
[After measurements](after.json) preserve the final comparison.

| Workload | Before Raster | After Raster | Change | After Changing FPS |
| --- | ---: | ---: | ---: | ---: |
| Queue auto-scroll | 10.145 ms | 10.711 ms | +5.6% | 58.63 |
| Settings auto-scroll | 10.059 ms | 10.252 ms | +1.9% | 58.59 |
| Boosted tilted Ink | 6.280 ms | 5.632 ms | -10.3% | 58.59 |
| Queue finger replay | 10.126 ms | 10.877 ms | +7.4% | 58.61 |
| Settings finger replay | 10.593 ms | 10.820 ms | +2.1% | 58.57 |

These are Slint raster times, separate from the native Ink simulation and composition.
Boosted tilted Ink's animation/composition interval is 5.326 ms before and 5.331 ms after.
Those intervals include synchronization with the second core and are not isolated CPU instruction times.

There is no loss of delivered panel cadence in these windows.
Queue and Settings consume slightly more raster time, while Ink rasterization is faster.
The panel itself measures approximately 58.6 Hz during final qualification.
The small FPS difference from the earlier baseline follows the measured panel ticks; there are no skipped animation phases.

A [warm repeated sequence](warm-replay.json) also passes all five workloads.
Queue raster time varies from approximately 10.7 to 11.1 ms across these runs and prior-page histories.
The measurements therefore establish the retained frame budget, rather than a cycle-exact attribution to an individual upstream commit.

## Integration Fixes

- Adapt generic RGB565 copying and its parity tests to the upstream removal of `TargetPixel::from_rgb565`.
  Keep the specialized native-endian and big-endian bulk-copy paths.
- Reconcile stale dependency references in the demo lockfile so the firmware builds with `--locked`.
- Release Ink buffers before sidebar navigation or a profiling replay constructs the next page.
  The kiosk controller already used this ordering.
  Before this fix, two qualification attempts stalled while switching from tilted Ink to Queue and recovered into BOOTSEL.
  Live sampling showed approximately 1 KiB of free heap at the stall.
  Early release removes that allocation overlap without changing the UI or particle counts.
- Move 4 KiB from the fixed heap reservation to the main stack.
  The old split left only 2188 bytes of measured stack headroom during Settings dragging with the new controls.
  The heap reservation is now 310 KiB, and the linker requires at least 24 KiB for the main stack.
  The linked profiling image reserves 25,156 bytes for the main stack and retains the dedicated 8 KiB core 1 stack.

The native gradient controls and press feedback match the PR source.
The Pico presentation pipeline, tilt fusion, orientation hysteresis, particle sizes, boost counts, and kiosk behavior are retained.
Ink release also clears its previous layout dimensions.
A new entry waits for the new Ink page layout before allocating vials, preventing allocation against stale geometry while the previous page remains alive.
The host lifecycle check fails with the old ordering and passes with this reset.
Temporary panic-location diagnostics are absent from the final source.

## Verification

All 35 particle, orientation, fusion, packet, and dual-core host tests pass.
All 35 software-renderer tests pass, including RGB565 bulk-copy parity, rounded rectangles, and the upstream gradient-dithering tests.
The headless UI check passes the actual kiosk checkbox, sequence, touch-to-exit overlay, and immediate Ink release through both sidebar destinations.

The broader `slint` partial-renderer integration target could not run because its Skia development dependency attempted source setup outside the sandbox.
The targeted software-renderer and printer UI checks above completed successfully.

The final ARM ELF was disassembled.
The RGB565 fill and big-endian copy routines remain in executable SRAM at `0x2000607c` and `0x20006124`.
The copy loop uses word loads and stores with `rev16` to swap two RGB565 pixels per instruction.
The linked code and read-only data reported by `llvm-size` decrease from 1,805,844 to 1,634,152 bytes, a reduction of 171,692 bytes.

Five display-memory readbacks pass with zero mismatches, including Queue and Settings in both orientations and Ink at rotation 0.
A combined capture sequence recovered into BOOTSEL before completing the sixth, flipped Ink readback.
The continuous workload qualification in both orientations passes; the combined capture failure remains a diagnostic limitation.
The saved framebuffer images verify the generated UI for the completed captures.
They do not constitute an optical test of the physical panel or a manual finger test.

## Complete Workload Qualification

The [complete workload run](all-workloads.json) covers eleven ten-second measurement windows at rotation 0.
Every window delivers one changing frame per panel tick, within the one-frame reporting boundary allowance.
All windows record zero missed phases, late strips, and scan errors.

| Workload | Changing FPS | Slint Raster |
| --- | ---: | ---: |
| `auto` | 58.68 | 10.713 ms |
| `pan-fast` | 58.68 | 10.591 ms |
| `poll-pan` | 58.68 | 10.616 ms |
| `settings-auto` | 58.63 | 10.254 ms |
| `settings-pan` | 58.64 | 10.680 ms |
| `settings-poll-pan` | 58.63 | 10.525 ms |
| `ink` | 58.73 | 5.760 ms |
| `ink-boost` | 58.63 | 5.648 ms |
| `ink-touch` | 58.64 | 5.650 ms |
| `ink-tilt` | 58.62 | 5.708 ms |
| `ink-tilt-boost` | 58.63 | 5.631 ms |

## Queue and Settings Hotspots

A temporary control worktree restores the old bitmap UI and its shared assets on the same current renderer and Pico backend.
Both detailed-counter images use the same 302 KiB heap reservation to leave room for profiling instrumentation in SRAM.
Neither image is the shipping configuration.
Only Queue and Settings are measured in this control; Ink is not allocated with that temporary heap split.

The [native UI counters](native-ui-deep.json) and [old UI counters](old-ui-deep.json) isolate the UI structure and primitive changes.

| Stage | Old UI Queue | Native Queue | Old UI Settings | Native Settings |
| --- | ---: | ---: | ---: | ---: |
| Whole raster | 10.650 ms | 11.044 ms | 10.427 ms | 10.680 ms |
| Dirty-region walk | 1.853 ms | 2.087 ms | 1.476 ms | 1.592 ms |
| Text dispatch and painting | 3.496 ms | 3.463 ms | 1.607 ms | 1.639 ms |
| Texture work | 2.161 ms | 2.165 ms | 2.045 ms | 1.009 ms |
| Rectangle work | 2.690 ms | 2.678 ms | 3.239 ms | 4.388 ms |
| Rounded rectangles | 2.279 ms | 2.274 ms | 2.732 ms | 2.736 ms |

The Queue raster delta is 0.394 ms, including 0.234 ms in dirty-region traversal.
Painted rectangle and rounded-rectangle call counts are unchanged, and their timing differences are small.
The added native controls cost time during dependency traversal even when their pixels do not need repainting.

The Settings raster delta is 0.253 ms.
Rectangle work grows by 1.149 ms while texture work falls by 1.035 ms, consistent with replacing bitmap backgrounds with native gradients.
Dirty-region traversal adds 0.116 ms.
Rounded-rectangle painting is almost unchanged.
Both controls report zero prepared-text cache misses during these warmed windows.

These are instrumented measurements, and nested scopes overlap.
Their relative costs locate work; they are not additive estimates of production frame time.

The most relevant follow-up experiments are skipping clean control subtrees during dirty traversal and specializing opaque dithered-gradient spans.
Both should preserve pixel output and native control behavior.
Increasing the text cache or changing the display SPI clock does not address the measured hotspots.

### Uninstrumented UI Control

The [ordinary release control](old-ui-coarse.json) uses the current renderer with the previous bitmap UI.
It preserves the old UI's raster timings: Queue takes 10.113 ms versus 10.145 ms in the old-base baseline, and Settings takes 10.059 ms in both.
The complete native-control build takes 10.711 ms and 10.252 ms, respectively.
This control attributes the practical increase to the UI changes rather than a measurable upstream renderer slowdown.
Small differences between individual runs should not be interpreted as renderer improvements.

The temporary control uses 302 KiB of reserved heap in both ordinary and instrumented builds.
The retained firmware uses 310 KiB.
The control only measures Queue and Settings and does not change CPU, display, or flash clocks.
