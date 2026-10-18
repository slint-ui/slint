# Board Transfer Deletion Audit

The audit covers the SDK-inspired changes after `63401e3c1a`.
It leaves the firmware source, UI and particles unchanged.

## Removed

- Seven intermediate code-generation reports repeat the same 24 artwork records.
  Each now refers to the complete final artwork table after an exact comparison of names, addresses, sizes and hashes.
  Build hashes, SRAM addresses, stack sizes and disassembly samples remain local to each report.
- The UF2 family check uses a set instead of importing a counter whose counts are never used.
- The top-level report no longer contains a temporary publication-status statement.

The original layout report and its separate rebuild retain complete artwork tables to demonstrate stable placement.
Per-change timing measurements, final pixel checks and negative trial results remain available.
Rejected implementation code is already absent; its short reports explain why those approaches should not be repeated blindly.

## Retained After Inspection

| Code | Reason |
| --- | --- |
| SPI transport fallback | Panel initialization uses SPI, and the optional 240 MHz configuration does not enable PIO |
| Cross-core doorbell routing | Timer callbacks can wake the display executor from core 0; only core 1 may poll it |
| Packet ownership and wakeups | DMA must retain immutable bytes, with registration before ownership checks to avoid lost wakeups |
| Scan-read pin restoration | Reads temporarily take pins from the active transport, including PIO |
| Capture release acknowledgment and read suspension | Diagnostics must finish the cross-core handoff and exclude concurrent panel reads |
| Packing bounds and unaligned fallback | The fast path needs checked strided bounds; other alignments still require the generic path |
| Main-stack guard and complete preceding-image verification | Neither guard is redundant with successful timing runs |

No runtime deletion is justified by this audit.
Firmware validation remains attached to the identical qualified binary rather than a new optimization trial.

## Recheck

The cleaned history retains the same firmware source as the qualified image.
Live uncached reads match all 1,639,672 checked flash bytes at 150 MHz CPU and 75 MHz flash.
Three new ten-second windows match panel cadence with zero missed phases, late strips and scan errors.
Queue measures 58.47 FPS, Settings 58.51 FPS and boosted tilted Ink 58.61 FPS.
Both scrolling runs use the automatically flipped orientation; Ink returns to the original orientation during the run.
These repeat measurements are validation, not evidence of a speedup from the deletion audit.

Four Queue and Settings framebuffer hashes equal the retained qualification captures.
Both panel-memory checks report zero differing pixels out of 76,800.
All 55 host tests pass: 42 compositor and kiosk tests, ten packing, ownership and scan-budget tests, and three UF2 tests.
Shell syntax and Python lint checks pass.
See [audit checks](audit-checks.json) for the image identity and measured counters.
The device is left on interactive Ink with live tilt and automatic orientation.
