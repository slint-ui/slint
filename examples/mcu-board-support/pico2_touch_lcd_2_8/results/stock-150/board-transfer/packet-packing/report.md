# Once-Bounded Packet Packing

The aligned eight-pixel row path checks the complete strided source extent once.
Each row then uses four native word loads and stores without repeated slice bounds checks.
The fallback preserves byte alignment, odd strides and other rectangle widths.

| Workload | Previous Packing | New Packing | Reduction | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 0.530 ms | 0.323 ms | 39% | 0 / 0 |
| Settings | 0.732 ms | 0.436 ms | 40% | 0 / 0 |
| Boosted tilted Ink | 0.597 ms | 0.351 ms | 41% | 0 / 0 |

All four windows meet physical panel cadence with zero scan errors.
Four Queue/Settings captures match exactly, and both panel-memory checks match all 76,800 pixels.
Three independent wire-format tests exercise every RGB565 value, independent source/destination alignments, odd strides and complete frame reconstruction.
Disassembly confirms native aligned word operations in the fast path; the generic fallback retains its copy helper.
The artwork, hardware-float and stack checks pass.
