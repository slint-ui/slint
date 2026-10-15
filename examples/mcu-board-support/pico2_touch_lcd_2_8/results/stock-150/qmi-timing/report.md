# QMI Timing Trials

Each trial retains two ten-second windows per scene after two seconds of settling per window.
The CPU remains at 150 MHz, quad-I/O flash at 75 MHz, and display SPI at 48 MHz in the first two configurations.
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
