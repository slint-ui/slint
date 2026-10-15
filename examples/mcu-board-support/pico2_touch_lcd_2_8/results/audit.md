# Pico Printer Audit and Regression Checks

The final stock-speed firmware preserves native 320 × 240 output and the existing printer UI.
Slint owns layout, text, controls, scrolling, input dispatch, and software rasterization.
Custom Rust composes each complete Ink vial image, including glass, liquid, and particles.
Slint displays those images and draws the surrounding UI.

The [tilt audit and qualification](stock-150/tilt/report.md) covers the subsequent fusion, hardware flip, memory sharing, and vial-label changes.
The measurements below describe the earlier boost qualification before two-core tilt support.

## Deletion Audit

- Removed the optional sent-pixel mirror, its second 153,600-byte framebuffer, and its USB and host-tool commands.
  Native captures and direct controller-memory checks provide the retained validation paths.
- Removed transient Settings geometry counters and their repeated property reads.
- Removed unused spelling exceptions and restricted binary-art exclusions to the actual asset directory.
- Removed intermediate measurements, rejected compiler and renderer trials, superseded screenshots, and stale reports.
  Retained final qualification data, native pixel references, and the current optimization guide.
- Excluded transfer timing reads and diagnostic frame identifiers from ordinary firmware builds.
- Removed the unused CPU clock query after enabling boost at both supported clocks.
- Replaced the test-only 52,224-byte atlas with direct-reference tests that exercise the actual row shader.
- Removed superseded normal-only qualification files and intermediate boost result directories.
  Kept current evidence and the compact optimization comparison.

The development evidence was archived outside the repository before deletion.
It is not part of the rewritten branch.

## Simplification Audit

- Moved benchmark replay from application setup into `queue_profile.rs`.
- Shared USB discovery, commands, mode selection, and statistics parsing through `printer_protocol.py`.
- Merged scroll bounds into the existing Queue and Settings motion globals.
- Shared watchdog feeding between stock and overclocked builds while preserving their distinct recovery policies.
- Named the selected internal features for their retained embedded behavior.
- Reused one Ink resource-release method for inactive animation, navigation, and Kiosk transitions.
- Measured Kiosk Ink with its actual animation phase instead of the separate page-controller timer.
- Use the same boost stress fast set at both clocks and remove the obsolete CPU-dependent host selection.
- Remove unused swap and duplicate scan-period snapshot slots from profiling reports.
- Remove a redundant profiling condition around CRC timing.
- Carry the checked boosted-particle age into shading instead of reading and unwrapping its birth twice.

The extended regression run exposed a stall while returning from Ink to Queue.
Ink retained roughly 110 KiB of animation buffers until a later timer callback, leaving about 15 KiB of free heap.
Kiosk now releases those buffers before constructing the next page.
Navigation also releases Ink images when changing pages or opening a subpage.
A host assertion checks that leaving Ink clears the image model.
The watchdog recovered the stalled build to ROM BOOTSEL; the replacement was flashed without pressing a button.
Four complete cycles passed after the ownership change.
The frozen final image then passed two further complete cycles and the full input sweep.

## Final Device Qualification

The [qualification report](stock-150/boost-qualification/report.md) contains nine input workloads and two complete Kiosk cycles.
Input workloads settle for two active seconds and retain ten seconds.
Kiosk retains seven-second windows inside its real ten-second views.
Every retained window passes the strict frame-count, phase, late-strip, scan-error, clock, and stack checks.
The CPU remains at 150 MHz and SPI at 48 MHz.
Normal Ink retains 16/26/20/32 particles; all-vial boost retains 26/42/32/52.

- [Twelve controller-memory comparisons](stock-150/boost-qualification/panel.json): all 76,800 pixels match in each check.
- [Ink title and percentage regions](stock-150/ink-text-parity.json): pixel-identical to the native reference.
- [Six adjacent-offset reference captures](stock-150/reference-parity.json): zero changed pixels in Queue and Settings.
- [On-device Kiosk interactions](stock-150/kiosk-widget.json): checkbox, ten-second page timer, exit on press, and stopped timers pass.
- [Independent particle pixels](stock-150/boost-qualification/pixel-parity.json): 19,200 frames match the original renderer.
- [Firmware metadata](stock-150/boost-qualification/firmware.json) identifies the qualified binary, toolchain, and memory layout.
- Disassembly confirms SRAM placement of hot texture, border, text, display, and particle routines.
  Native word operations and byte swaps remain present.

## Host Validation

- Core library: 382 tests pass with the embedded geometry cache enabled.
- Software renderer: 30 tests and one documentation test pass.
- Native Ink and Kiosk motion: 15 tests pass, including direct-reference shading and cached restoration across holds, releases, and level changes.
- Packet ownership, tile planning, packing, scan budgets, touch decoding, run isolation, and Kiosk motion: 21 tests pass.
- Native Kiosk UI check passes with the embedded geometry cache enabled, including resource release, visible Ink text, and no click-through.
- Stock release, profiling release, and optional 240 MHz release builds link successfully.

The broader API partial-renderer suite could not build offline because its Skia dependency attempted to fetch unavailable sources.
It was not counted as passing.
Physical touch-sensor latency and optical scan coherence were not measured during this audit.
Controller memory comparisons and counters cannot establish the physical display's appearance.
Warmed throughput windows exclude page construction and full-screen transitions; repeated cycles exercise their functional stability.

## History

The published history contains only selected implementations, their tests, generated artwork, reproducible tools, and current qualification evidence.
Superseded normal-only qualification and intermediate boost records are archived outside the repository.
The final report retains their compact timing comparison.
History reconstruction must preserve the final audited tree exactly.
The rebuilt image is verified again on the device before replacing the remote branch.
