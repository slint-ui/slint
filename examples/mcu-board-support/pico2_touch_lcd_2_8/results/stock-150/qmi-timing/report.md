# QMI Timing Trials

<!-- cSpell:ignore PAGEBREAK -->

The baseline, chip-select, burst, and 133 MHz trials retain two ten-second windows per scene.
Each window follows two seconds of settling.
The 150 MHz flash trial and restored configuration each retain one ten-second window per scene.
The CPU remains at 150 MHz, quad-I/O flash at 75 MHz, and display SPI at 48 MHz in the first three configurations.
Rotation is locked during throughput measurements.
Both orientations are checked separately against controller memory.
The UI and all particle sizes and counts remain unchanged.

## Shorter Chip-Select Gap

The retained change sets `MIN_DESELECT` from 7 to 1.
The documented QMI formula reduces the minimum inactive interval from approximately 53.3 ns to 13.3 ns.
The W25Q128JV specifies a 10 ns minimum inactive interval for reads.
Timing updates execute in SRAM with interrupts disabled before core 1 starts.
The QMI enters direct mode and drains its current memory transfer before non-divider fields change.
Disassembly confirms no flash instruction fetches occur while memory access is disabled.

The baseline and candidate have identical flash code and artwork sections.
Their SRAM initialization images differ by one byte: the chip-select timing constant.
This comparison isolates the timing change from renderer code placement.

| Stage | Gap 7 | Gap 1 | Reduction |
| --- | ---: | ---: | ---: |
| Queue raster | 10.219 ms | 10.103 ms | 1.13% |
| Settings raster | 10.262 ms | 10.147 ms | 1.13% |
| Boosted tilted Ink raster | 6.403 ms | 6.314 ms | 1.38% |
| Boosted tilted Ink composition | 5.322 ms | 5.288 ms | 0.62% |

All twelve retained windows pass panel cadence with zero missed phases, late strips, and scan errors.
All twelve controller-memory checks match every framebuffer pixel.
Frozen Queue and Settings captures at offsets 12 and 13 match the previous 75 MHz qualification exactly.
Initial flash checks and two retained end-of-run passes per image match host section hashes through uncached reads.
The measured live registers confirm the requested timing fields.
These are read-integrity, framebuffer, and controller-memory checks; live scan quality is not optically measured.
Voltage and temperature margins are not swept.

- [Baseline measurements](baseline.json)
- [Shorter-gap measurements](cs1.json)
- [QMI timing specification](https://github.com/raspberrypi/pico-sdk/blob/master/src/rp2350/hardware_regs/include/hardware/regs/qmi.h)
- [Flash timing specification](https://www.winbond.com/resource-files/w25q128jv%20revg%2004082019%20plus.pdf), section 9.6

## Sequential Burst Boundaries

The next retained change clears `PAGEBREAK`, removing the forced restart at each 1 KiB boundary.
CPU, flash clock, read format, receive delay, and the shorter chip-select gap stay unchanged.
The flash code and artwork sections remain byte-identical to the preceding build.
The SRAM image differs by one byte in the startup timing mask.

| Stage | Forced 1 KiB Boundary | No Forced Boundary | Time Saved |
| --- | ---: | ---: | ---: |
| Queue raster | 10.103 ms | 10.096 ms | 7.8 µs |
| Settings raster | 10.147 ms | 10.140 ms | 6.9 µs |
| Boosted tilted Ink raster | 6.314 ms | 6.304 ms | 10.5 µs |
| Boosted tilted Ink composition | 5.288 ms | 5.295 ms | -6.6 µs |

The raster gain is small: about 7 microseconds in Settings and 11 in tilted Ink.
Those repeated raster ranges do not overlap the preceding configuration's ranges.
Queue ranges overlap, and composition varies more than its mean change, so those differences are inconclusive.
This is a small read-latency improvement rather than a substantial frame-budget gain.
All six retained windows pass panel cadence with zero missed phases, late strips, and scan errors.
Both panel orientations, exact frozen Queue and Settings pixels, and uncached flash hashes pass again.
See the [raw burst-boundary checks](burst.json).

## 133 MHz CPU and Flash Trial

This trial uses a 133 MHz CPU and flash divisor 1, retaining the preceding chip-select and burst settings.
The system PLL uses 12 MHz × 133 / 6 / 2 = 133 MHz.
The independent peripheral PLL preserves 48 MHz display SPI, USB, and ADC clocks.
Receive timing and the read format stay unchanged.
All six windows match panel cadence with zero missed phases, late strips, and scan errors.
Both controller orientations, frozen Queue and Settings pixels, and uncached flash hashes also pass.

| Stage | CPU 150 / Flash 75 MHz | CPU 133 / Flash 133 MHz | Time Change |
| --- | ---: | ---: | ---: |
| Queue raster | 10.096 ms | 9.833 ms | -2.60% |
| Settings raster | 10.140 ms | 9.597 ms | -5.35% |
| Boosted tilted Ink raster | 6.304 ms | 5.933 ms | -5.88% |
| Boosted tilted Ink composition | 5.295 ms | 5.531 ms | +4.46% |

The faster flash lowers raster time, but vial composition is about 4.5 percent slower.
Ink's total measured draw-plus-composition path changes from 12.893 to 12.860 ms, a 33-microsecond mean improvement within observed variation.
Queue and Settings total draw paths grow by approximately 0.111 and 0.151 ms because packet waiting and CPU-side preparation take longer.
Their raster work falls, but their presentation cadence remains unchanged.
The 133 MHz pair therefore does not establish a clear overall frame-budget improvement across the three workloads.
The retained default stays at 150 MHz CPU and 75 MHz flash; the trial's clock code is removed.
See the [complete trial measurements](clock133.json).
The flash and SRAM section sizes remain unchanged, but flash instruction generation differs in the clock trial.
The SRAM image changes only in clock setup and diagnostic location data; this trial is a full-build comparison.

## Single 150 MHz Flash Trial

The CPU stays at 150 MHz and the flash divisor changes from 2 to 1.
Receive delay, read format, chip-select gap, and burst settings stay unchanged.
The flash code and artwork are byte-identical to the restored 75 MHz build.
Their SRAM initialization images differ by one byte: the divider constant.

| Stage | Flash 75 MHz | Flash 150 MHz | Reduction |
| --- | ---: | ---: | ---: |
| Queue raster | 10.096 ms | 8.662 ms | 14.20% |
| Settings raster | 10.140 ms | 8.519 ms | 15.99% |
| Boosted tilted Ink raster | 6.304 ms | 5.291 ms | 16.07% |
| Boosted tilted Ink composition | 5.295 ms | 4.801 ms | 9.34% |

All three ten-second windows match panel cadence, approximately 58.6 Hz.
They record zero missed phases, late strips, and scan errors.
Both panel orientations match controller memory, and frozen Queue and Settings pixels match the reference captures exactly.
Initial uncached section checks and two end-of-run hash passes match the host ELF.
Each integrity pass checks 1,805,844 bytes across five loaded sections.
See the [single-trial measurements](flash150.json).

The 150 MHz setting exceeds the W25Q128JV's 133 MHz rating.
A short passing run does not establish operating margins, long-term stability, or physical scan quality.
The trial clock change is removed, and the device is restored to 75 MHz flash after testing.

## Retained Configuration

- CPU: 150 MHz.
- Quad-I/O flash: 75 MHz, divisor 2, receive delay 2.
- Read chip-select deselect setting: 1.
- Forced flash page boundary: disabled.
- Display SPI: 48 MHz.

The two timing improvements remain separate commits.
Both clock experiments are absent from the retained source.

The [restoration checks](final.json) confirm the retained 75 MHz configuration after the 150 MHz trial.
All three ten-second windows pass panel cadence with zero missed phases, late strips, and scan errors.
Both panel orientations, frozen reference pixels, and all uncached flash hashes pass again.
Restored raster times are 10.345 ms for Queue, 10.196 ms for Settings, and 6.317 ms for boosted tilted Ink.
Ink composition takes 5.283 ms.
The comparison table uses the earlier two-window baseline; restored Queue varies by 0.250 ms from that mean.
The retained and trial binaries have identical flash code and artwork, with only the divider byte differing in their SRAM images.

Normal and profiling release builds succeed, and all 35 host tests pass.
The [firmware metadata](firmware.json) records build hashes, clock settings, and memory headroom.
The device is left in live-sensor Ink mode with automatic orientation enabled.
