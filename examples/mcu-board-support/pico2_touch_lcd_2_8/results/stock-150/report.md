# Stock-Speed Qualification

The initial stock-speed qualification used the rated 150 MHz CPU, 48 MHz SPI, and native 320 × 240 output.
The target is the measured panel cadence, approximately 58.6 Hz.
The [75 MHz flash qualification](flash-75/report.md) records its isolated raster savings and repeated corruption checks.
The [QMI timing trials](qmi-timing/report.md) document the retained shorter chip-select gap, uninterrupted bursts, and clock comparisons.
The [latest touch timing qualification](touch-panning/input-phase/report.md) uses 150 MHz CPU, 75 MHz flash and 60 MHz display SPI.
Its panel-derived input clock removes the Queue tail-latency regression while retaining panel cadence and unchanged pixels.
The Settings list includes the final Kiosk mode checkbox and a 207-pixel scroll range.
Normal Ink retains 16/26/20/32 particles; both clock configurations now enable held-vial boost to 26/42/32/52.

The [latest Ink qualification](tilt/report.md) covers normal, boosted, and tilted boost animation with the larger labels and automatic orientation support.
Those three windows match panel cadence with zero missed phases, late strips, and scan errors.
Both hardware orientations pass full controller-memory comparisons.

Before tilt support, all nine [input workloads](core1-ink/report.md) and two complete Kiosk cycles passed with the same zero-drop guards.
That qualification composes two vials on each core and removes the outer corner overlay.
It includes repeated all-vial stress, hold/release replay, native pixel parity, and direct panel-memory checks.
See the [audit](../audit.md) for retained changes and regression coverage.
The [latest firmware metadata](qmi-timing/firmware.json) records the stock-speed Ink binary and measured memory headroom.

Raster timings exclude input dispatch, packet planning, packing, checksum work, and transport waiting.
Pointer replay tests the real Slint widgets and backend dispatch; it does not measure physical finger latency.
Page construction and full-screen transitions are outside the warmed throughput windows.
The scan-group allowance remains 160 microseconds, with the original beam clearances and 700-microsecond admission margin.
