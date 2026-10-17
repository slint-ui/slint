# Independent Ink Surface Motion

Each vial has a separate damped response to the fused gravity target.
Damping ratios are 0.68, 0.80, 0.94, and 1.08, from yellow to black.
The response converges to the same resting gravity angle, limited to ±30 degrees.
The restoring strength uses the depth factor `tanh(1.841 * depth / radius)` with a bounded rational approximation.
Shallow liquid responds more slowly.
The model approximates surface motion without simulating a fluid grid.

A second damped response converts changes in bulk motion into a visible settling ripple, bounded to 3 pixels.
Each vial retains faint surface motion after the larger tilt-driven ripple settles.
Two spatial modes use separate seeded noise streams, phases, and target intervals for each vial.
Cubic interpolation joins successive targets smoothly; random values are not chosen independently every frame.
The target intervals span 73–146 panel ticks, depending on vial and mode.
The noise is deterministic for repeatable tests and has no short repeating animation loop.

Idle coefficients are bounded to 140 and 39 in Q8 units.
Their combined displacement cannot exceed approximately 0.70 pixels.
The existing tilt-driven ripple remains bounded to 3 pixels before the idle motion is added.
The average surface angle, curved meniscus, highlights, gravity direction, bubble sizes, and populations retain their existing behavior.
Both spatial modes keep the contact points fixed and balance displaced area within fixed-point rounding.
Bubble fading and retirement follow the combined local surface.

The new target interpolation runs once per vial per panel tick.
The additional spatial mode uses integer arithmetic in the existing surface renderer.
It introduces no dynamic allocation or per-pixel trigonometry.
Both coefficients are included in the retained geometry key so quiet surfaces continue repainting correctly.

## Qualification

CPU remains at 150 MHz, quad flash at 75 MHz, and display SPI at 48 MHz.
All ordinary workloads use ten-second measurement windows after two seconds of settling.
The stationary before-and-after runs force upright gravity to isolate idle surface cost.
Moving workloads use the existing tilt replay, and abrupt reversals alternate between tilt limits every 250 ms in flipped landscape.

| Workload | Changing FPS | Slint Raster | Animation and Composition | Missed Phases | Late Strips | Scan Errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Stationary Ink | 58.60 | 5.654 ms | 4.457 ms | 0 | 0 | 0 |
| Stationary Ink, all four boosted | 58.60 | 5.579 ms | 6.260 ms | 0 | 0 | 0 |
| Queue auto-scroll | 58.58 | 11.198 ms | 0.000 ms | 0 | 0 | 0 |
| Settings auto-scroll | 58.55 | 10.962 ms | 0.000 ms | 0 | 0 | 0 |
| Moving Ink | 58.56 | 5.641 ms | 5.614 ms | 0 | 0 | 0 |
| Moving Ink, all four boosted | 58.56 | 5.595 ms | 7.150 ms | 0 | 0 | 0 |
| Touch hold and release | 58.56 | 5.642 ms | 5.329 ms | 0 | 0 | 0 |
| Boosted abrupt reversals, flipped | 58.57 | 5.588 ms | 7.039 ms | 0 | 0 | 0 |

Every completed window matches panel ticks within the one-frame reporting boundary allowance.
The all-vial boosted populations remain 26, 42, 32, and 52.
Minimum measured main-stack headroom is 8,820 bytes and free heap is 7,632 bytes.

The stationary normal Ink animation/composition measurement increased from 3.129 to 4.457 ms.
Stationary boosted Ink increased from 5.091 to 6.260 ms.
These aggregate timings include synchronization with the second core and workload variation; they do not isolate the noise instructions.
The added cost includes continuously rebuilding and repainting otherwise resting surfaces.

All 42 host tests pass.
They cover noise bounds, smoothness, independent vial trajectories, balanced area, fixed contact points, curved surface continuity, retained-buffer parity, and real-thread vial parity.
The headless UI integration executable also passes the kiosk sequence, immediate exit, and page-entry allocation checks.
Disassembly with `llvm-objdump --mcpu=cortex-m33` confirms hardware floating-point arithmetic remains in the slosh path.

The [device framebuffer capture](device-idle-0.png) has a verified transfer CRC and was inspected alongside host-rendered idle frames.
This verifies rendered composition, not physical panel appearance during motion.
Capture pauses are outside the timing windows.

[Stationary baseline](before-idle.json), [idle results](idle.json), [moving and touch results](moving.json), [reversal stress](reversals.json), and [firmware metadata](firmware.json) retain the evidence.

The device is left in manual Ink with live IMU input and automatic orientation for physical evaluation.

## Audit

The deletion audit removes superseded qualification reports and a redundant device capture.
The final report retains the useful stationary baseline, current workload measurements, and firmware identity.
Measurement files retain qualification counters and primary timings; unused deep-instrumentation and lifetime counters are omitted.
Redundant comments and a direct cache-key assertion were removed.
Retained-buffer pixel-parity tests continue to verify cache behavior.

The simplification audit groups the mean surface plane and both ripple coefficients in one `Surface` value.
Rendering, bubble clipping, and cache invalidation share that value.
The frequency calculation is outside the bounded integration loop.
The liquid motion parameters and particle populations are unchanged.

A before-and-after host replay renders 900 frames across four vials, including tilt, boost, release, and fill-level changes.
All 48,384,000 little-endian RGB565 output bytes match exactly.
The SHA-256 fingerprint is `38f412ebb26b2c1a9dac33b293feb1f1999e9113dc2f875ca73db32e22ffbb4b`.

The eight incremental liquid-motion commits are replaced by three focused commits for settling motion, idle variation, and qualification.
