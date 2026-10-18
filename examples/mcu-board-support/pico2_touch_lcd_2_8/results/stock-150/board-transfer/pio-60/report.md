# PIO Display Output at 60 MHz

A two-instruction PIO0 transmitter supplies the same immutable DMA packets at 60 MHz instead of 48 MHz SPI.
Commands use eight-bit pulls; pixels use sixteen-bit pulls with the existing halfword byte swap.
A TXSTALL check waits for the final clock edge before chip select changes.
GPIO scan reads restore the previous pin controls, preserving either SPI or PIO routing.
The 240 MHz feature keeps its previous SPI path.

| Workload | Previous Transfer Busy per Frame | PIO Transfer Busy per Frame | Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 9.064 ms | 7.246 ms | 58.59 | 0 / 0 |
| Settings | 12.440 ms | 9.982 ms | 58.61 | 0 / 0 |
| Boosted tilted Ink | 10.391 ms | 8.326 ms | 58.69–58.71 | 0 / 0 |

Wire-transfer busy time falls approximately 20 percent.
This excludes beam-following waits and does not imply the same reduction in raster time.
The repeated Ink window also passes, and every window records zero scan errors.
Four Queue/Settings captures match exactly, and both panel-memory comparisons match all 76,800 pixels.
All 24 artwork symbols retain their bytes and addresses.
The static main-stack reservation is 30,524 bytes.

The physical panel still determines animation cadence.
A complete RGB565 frame takes at least 20.48 ms on a 60 MHz single-lane link; this path reaches cadence through partial updates.
Software timing and panel-memory checks do not constitute an optical recording of scanout.
