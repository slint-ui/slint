# CPU-Scaled Panel Read Delay

Panel reads use 30 NOP delay cycles at 150 MHz and retain 48 cycles at 240 MHz.
Both delays contribute 200 ns per half-clock before GPIO instruction overhead.
This preserves the qualified delay duration while avoiding unnecessary polling work at stock CPU speed.
The scan margins, read protocol and physical panel configuration are unchanged.

Boosted tilted Ink composition falls from 7.951–7.979 ms to 7.621–7.663 ms.
Its raster cost remains approximately 5.9 ms.
Queue and Settings packet waiting fall by approximately 0.05 ms and 0.10 ms respectively.
Their raster times change by +0.02 ms and +0.08 ms; this is a polling reduction rather than a renderer optimization.
All four windows meet panel cadence with zero missed phases, late strips and scan errors.
Four Queue/Settings captures match the preceding image exactly, and both panel-memory checks match all 76,800 pixels.
All artwork and binary checks pass.
