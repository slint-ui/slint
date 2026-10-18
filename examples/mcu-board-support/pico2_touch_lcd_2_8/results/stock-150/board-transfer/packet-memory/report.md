# Packet Memory Budget

Each of the three immutable packet slots stores 30 KiB instead of 32 KiB.
Both capacities hold eight complete 8-by-240 RGB565 strips.
This recovers 6 KiB without changing frame pixels, DMA ownership or rendering code.
Partial rectangles can change packet boundaries, so each representative workload is measured separately.

| Workload | Previous Raster | New Raster | Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 10.993 ms | 10.965 ms | 58.65 | 0 / 0 |
| Settings | 10.645 ms | 10.657 ms | 58.60 | 0 / 0 |
| Boosted tilted Ink | 5.840 ms | 5.848 ms | 58.58 | 0 / 0 |

All windows record zero scan errors.
The static main-stack reservation rises from 24,644 to 30,788 bytes.
The unchanged 24 KiB linker guard remains enabled.
The recovered space permits the PIO transport trial without shrinking the heap or weakening stack protection.
