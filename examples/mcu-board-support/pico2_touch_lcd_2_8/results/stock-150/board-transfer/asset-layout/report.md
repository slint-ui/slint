# Stable Printer Artwork Placement

The executable and RAM-load image occupy the first 2 MiB of flash.
Read-only assets begin at `0x10200000`, with named Ink and Slint image resources sorted before other read-only data.
All 24 named artwork resources retain their bytes and addresses after a separate source rebuild.
The main-core stack reservation remains above 24 KiB, and core 1 retains its dedicated 8 KiB stack.

Each workload uses a two-second warmup and a ten-second measurement window at fixed orientation.
Both images use 150 MHz CPU, 75 MHz flash and 48 MHz display SPI.

| Workload | Before Raster | After Raster | After Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 10.948 ms | 10.958 ms | 58.59 | 0 / 0 |
| Settings | 10.418 ms | 10.440 ms | 58.56 | 0 / 0 |
| Boosted tilted Ink | 5.584 ms | 5.636 ms | 58.57 | 0 / 0 |
| Boosted tilted Ink repeat | 5.583 ms | 5.641 ms | 58.57 | 0 / 0 |

All windows record zero scan errors.
Four Queue and Settings captures match the preceding build exactly.
Two panel-memory comparisons match all 76,800 pixels.
Uncached flash checks match 1,636,960 bytes from the normal image.
The code-generation check confirms SRAM placement of time-critical functions and hardware floating-point instructions.
The 42 host tests, three packet-slot tests and three wire-format tests pass.

Fixed asset addresses make future incremental updates less sensitive to executable growth.
They do not guarantee small updates: compiler-generated code and literals can still change extensively.
A temporary USB-name diagnostic preserved all artwork addresses but failed to start after both incremental and full flashing.
Its payload size is excluded from speed claims, and the diagnostic source was restored.
The normal layout build passes the timing and pixel checks above.
