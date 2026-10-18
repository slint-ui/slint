# Display Priority During Ink Composition

Core 1 runs the display task in a priority-2 interrupt executor.
DMA completion has priority 1 and USB has priority 3; particle work remains in the thread executor.
Core 0 routes cross-core executor wakeups through the SIO doorbell instead of polling the display executor itself.
Diagnostic panel-memory reads suspend idle scan sampling after the capture handshake drains all packets.

The preceding thread-executor trial observed up to 1,583 microseconds of rectangle transfer overhead beyond wire time.
The first interrupt-executor trial reduces that maximum to 59 microseconds across all four workloads.
It preserves zero missed phases, late strips and scan errors.
This bounds display interference from particle jobs rather than accelerating their arithmetic.
Ink composition wall time increases from approximately 7.17 ms to 8.26 ms because display work can interrupt it.

The final diagnostic-safe image repeats all four windows with zero misses, late strips or scan errors.
Queue raster is 10.993 ms at 58.68 changing FPS; Settings is 10.645 ms at 58.65 FPS.
Boosted tilted Ink raster is 5.840 ms at 58.60–58.63 FPS.
All artwork and four Queue/Settings captures match exactly, and both panel-memory checks match all 76,800 pixels.
The 24 KiB main-stack reservation remains intact.
