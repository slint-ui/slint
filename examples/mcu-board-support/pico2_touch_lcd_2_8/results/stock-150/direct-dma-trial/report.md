# Direct Framebuffer DMA Trial

The comparison uses 150 MHz CPU, 75 MHz flash, 60 MHz PIO, two seconds of warm-up and ten-second measurement windows.
The packet baseline includes the Ink cache-lifetime fix and native RGB565 texture hook.
The trial keeps the same UI, artwork, particle counts and scan deadlines.

The trial transfers ownership of one complete framebuffer to the display task.
A second DMA channel chains source-row addresses into the display DMA channel.
The renderer reacquires the framebuffer only after the display task releases it.
This prevents concurrent writes without allocating a second framebuffer.

| Workload | Packet FPS | Direct FPS | Packet Raster | Direct Raster | Packet Render/Publish | Direct Render/Publish | Direct Misses | Late Strips, Both |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| auto | 58.51 | 47.14 | 11.268 ms | 10.306 ms | 13.810 ms | 20.499 ms | 115 | 0 |
| settings-auto | 58.58 | 40.45 | 10.785 ms | 10.072 ms | 15.991 ms | 24.008 ms | 181 | 0 |
| ink-tilt-boost | 58.50 | 58.70 | 5.396 ms | 4.999 ms | 6.629 ms | 8.811 ms | 0 | 0 |
| ink-tilt-boost | 58.58 | 58.67 | 5.395 ms | 4.999 ms | 6.608 ms | 8.775 ms | 0 | 0 |

Raster excludes waiting to reacquire the direct framebuffer.
Render/publish includes that waiting, planning and the profiling checksum, but excludes Ink preparation and completion of the final display transfer.
Ink preparation remains approximately 7.2 ms in both builds.
The faster direct raster does not mean more usable frame headroom: direct Ink waits about 3.15 ms for ownership.
Its remaining interval is approximately 1.07 ms, versus 3.22–3.24 ms with packets.

Packet storage decreases by 94,256 bytes; the direct DMA task needs 1,000 more bytes.
The trial converts 90 KiB into additional general heap capacity, increasing it from 310 to 400 KiB.
Ink free heap rises from 12,432 bytes to approximately 104,800 bytes.
This is a RAM-saving option, rather than a throughput improvement for this application.

All packet windows have zero misses, late strips and scan errors.
Direct Ink matches panel cadence, but direct Queue and Settings miss 115 and 181 phases respectively.
Every direct window has zero late strips and scan errors.
Four fixed Queue/Settings frame captures match the baseline byte-for-byte, and both panel comparisons report zero mismatches across 76,800 pixels.
Those checks do not establish optical smoothness during motion.

The trial is discarded at the user's request.
The source and device return to packet DMA with the committed fragmentation fix.
The trial ELF and isolated patch remain outside the repository under `/private/tmp/printer-direct-dma-trial.*`.
