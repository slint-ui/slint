# Stock-Speed Qualification

The final firmware uses the rated 150 MHz CPU, 48 MHz SPI, and native 320 × 240 output.
The target is the measured panel cadence, approximately 58.6 Hz.
The Settings list includes the final Kiosk mode checkbox and a 207-pixel scroll range.
Normal Ink retains 16/26/20/32 particles; both clock configurations now enable held-vial boost to 26/42/32/52.

All nine [current input workloads](boost-qualification/report.md) and two complete Kiosk cycles pass with zero missed phases, late strips, and scan errors.
The boost qualification includes repeated all-vial stress, hold/release replay, native pixel parity, and direct panel-memory checks.
See the [audit](../audit.md) for retained changes and regression coverage.
The [current firmware metadata](boost-qualification/firmware.json) records the stock-speed binary and stack layout.

Raster timings exclude input dispatch, packet planning, packing, checksum work, and transport waiting.
Pointer replay tests the real Slint widgets and backend dispatch; it does not measure physical finger latency.
Page construction and full-screen transitions are outside the warmed throughput windows.
The scan-group allowance remains 160 microseconds, with the original beam clearances and 700-microsecond admission margin.
