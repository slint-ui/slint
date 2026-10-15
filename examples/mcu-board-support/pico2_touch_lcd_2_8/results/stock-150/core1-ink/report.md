# Compose Ink on Both Pico Cores

The CPU remains at stock 150 MHz, SPI at 48 MHz, and output at native 320 × 240.
Core 1 composes Yellow and Cyan while core 0 composes Red and Black.
Slint layout, input, image publication, and software rasterization remain on core 0.
Normal counts remain 16/26/20/32 and all-vial boost remains 26/42/32/52.
Bubble sizes, shading, trajectories, fractional coverage, birth timing, and retirement are unchanged.
The only intentional visual change is removal of the outer screen-corner overlay.

## Isolated Two-Core Gain

The fresh serial baseline and balanced two-core windows below both retain the rounded outer overlay.
Composition is wall time from animation preparation through worker join and image publication.
Slint raster is measured separately.
Neither column includes every event-loop or presentation stage.

| Workload | Serial Composition | Two-Core Composition | Composition Reduction | Serial Composition + Raster | Two-Core Composition + Raster |
| --- | ---: | ---: | ---: | ---: | ---: |
| Normal Ink | 3.193 ms | 2.863 ms | 10.4% | 11.391 ms | 11.338 ms |
| All-vial boost | 6.846 ms | 4.860 ms | 29.0% | 14.935 ms | 13.217 ms |
| Hold/release replay | 4.522 ms | 3.563 ms | 21.2% | 12.667 ms | 11.993 ms |

[Serial baseline](serial-baseline.json) and [balanced two-core measurements](two-core-rounded.json) retain the complete timing and cadence counters.
Boost saves 1.72 ms, or 11.5%, across composition and Slint raster.
Normal Ink has almost no net gain across these stages because raster rises by about 0.28 ms.
The cause of that raster increase was not isolated; code placement and memory contention remain possible explanations.

The Yellow and Cyan pair gives core 1 58 boosted bubbles while core 0 handles 94.
The optimal split depends on display work as well as particle count.

## Ownership and Scheduling

A scoped single-job queue transfers a Send closure borrowing disjoint plain pixel buffers and renderer state.
Acquire/release state changes publish the request and its completion.
A join guard waits before borrowed data can be reused, including if the local closure unwinds.
Slint objects are never sent to the worker or given unsafe Send implementations.
Core 0 reserves damage-vector storage before submission so the worker does not allocate.

Each worker call restores eight rows or stamps one particle, then yields to the core-1 executor.
Display payloads continue through DMA channel 0 using the existing immutable packet slots and scan admission rules.
The worker sleeps on an atomic waker when idle.
It does not poll a timer while Queue or Settings is active.
The implementation adds no full-screen framebuffer and retains two image buffers per vial.

The particle shader is shared by serial and bounded rendering and remains in executable SRAM at `0x2000f908` in the final image.
LLVM disassembly confirms native multiply-accumulate and hardware integer division instructions in the kernel.
The profiling restore and stamp totals sum active intervals on both cores and can overlap.
Use composition wall time to assess the critical path; do not add those totals as sequential durations.

## Corner Removal and Timing Tails

The balanced build passed all nine standalone workloads and three extra boost windows, but full Kiosk testing exposed one or two missed Settings phases.
There were no late strips or scan errors in those windows.
The slowest observed Settings frame spent 15.303 ms in raster and 5.004 ms waiting for packet space, totaling 22.229 ms across the measured draw path.
Average timings alone hid this scheduling tail.
The coarse USB report now retains the stage breakdown for the slowest frame without growing its report array.

The requested corner removal deletes the full-screen rounded-border primitive entirely.
All six Queue/Settings reference images change only 424 outer-corner pixels each; their central pixels remain identical.
After removal, standalone Settings raster falls from 12.752 to 12.143 ms and boosted Ink raster from 8.357 to 7.501 ms in these runs.
Queue remains close to 12.9 ms.
These are measured before/after build timings; they include any associated code-layout changes.
They should not be attributed to the two-core split.

## Final Input Windows

Each workload settles for two active seconds and retains ten seconds.
All nine windows pass strict panel-cadence checks with zero missed phases, late strips, and scan errors.

| Workload | Changing FPS | Slint Raster | Vial Composition | Missed Phases | Late Strips | Scan Errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| auto | 58.66 | 12.863 ms | 0.000 ms | 0 | 0 | 0 |
| pan-fast | 58.63 | 12.619 ms | 0.000 ms | 0 | 0 | 0 |
| poll-pan | 58.64 | 12.612 ms | 0.000 ms | 0 | 0 | 0 |
| settings-auto | 58.62 | 12.143 ms | 0.000 ms | 0 | 0 | 0 |
| settings-pan | 58.63 | 12.662 ms | 0.000 ms | 0 | 0 | 0 |
| settings-poll-pan | 58.62 | 12.544 ms | 0.000 ms | 0 | 0 | 0 |
| ink | 58.61 | 7.701 ms | 2.820 ms | 0 | 0 | 0 |
| ink-boost | 58.62 | 7.501 ms | 4.773 ms | 0 | 0 | 0 |
| ink-touch | 58.61 | 7.615 ms | 3.476 ms | 0 | 0 | 0 |

[Input measurements](inputs.json) retain panel ticks, completed presentations, particle counts, heap, stack, and stage timings.
Final normal Ink composition plus raster is 10.521 ms; boosted Ink is 12.274 ms; hold/release replay is 11.090 ms.
The boost reduction from the serial baseline is 2.66 ms across those two stages, including corner removal.
This is not an end-to-end CPU-utilization or latency measurement.

## Pixel and Host Checks

An independent frozen pre-change renderer matches the bounded renderer over 19,200 frames and 106,406,400 pixels with zero differences.
All 19 host tests pass, including bounded-paint parity, real-thread vial parity, borrowed-job lifetime, local-panic join, fill changes, and hold/release behavior.
The real-thread test compares both retained buffers against serial output through 900 frames for all four vials.
Host Kiosk assertions and software-rendered captures also pass after the corner removal.
The stock normal and profiling release configurations build successfully with ThinLTO and one code-generation unit.

## Final Transition and Delivery Checks

Two complete Kiosk cycles pass at 58.57–58.64 FPS, with zero missed phases, late strips, and scan errors in all six retained windows.
Three additional all-vial boost windows pass at 58.60–58.61 FPS with the same zero-drop guards.
Twelve panel-memory checks compare all 76,800 pixels each with zero mismatches across Queue, Settings, Ink, and boosted Ink.
The Kiosk checkbox, ten-second page timer, immediate touch exit, and stopped timer also pass device replay.

The lowest observed unused main stack is 7,372 bytes and the minimum reported free heap is 11,896 bytes.
The final link-time main-stack gap is 22,516 bytes, above the required 20,480 bytes.
Core 1 retains its separate 8 KiB stack.

- [Repeated boost](repeat-boost.json)
- [Kiosk windows](kiosk.json)
- [Panel-memory checks](panel.json)
- [Kiosk widget replay](widget.json)
- [Independent pixel comparison](pixel-parity.json)
- [Firmware metadata](firmware.json)

These checks establish generated pixels, completed changing-frame cadence, panel-memory delivery, and replayed input behavior.
They do not measure physical finger latency or optically capture the panel scan.

All four direct Slint vial press/release callbacks pass.
The all-vial hold reaches 26/42/32/52; after release, live extras remain and eventually drain to 16/26/20/32.
See the [release states](release.json) and [native boosted image](held.png).
The device is left on Ink in manual mode.

One initial hold-image transfer failed host CRC validation.
Four immediate diagnostic captures and the repeated held/released/drained captures subsequently matched their reported CRCs with complete 153,600-byte payloads.
The original failing payload and byte count were not retained, so the cause is unresolved.
The distributed app screenshots and retained images all passed CRC validation.
This capture-only failure is separate from the zero-drop cadence windows and zero-mismatch panel-memory checks.
