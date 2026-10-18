# Refresh-Aware Idle Sampling

Idle scan sampling waits up to 2 ms while far from the predicted refresh boundary.
It retains the previous 500-microsecond sampling bound near that boundary and while the period is unknown.
Packet publication still wakes the consumer immediately.
Active strip admission, beam margins and hardware wrap detection are unchanged.

| Workload | Previous Raster | New Raster | Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 10.686 ms | 10.447 ms | 58.71 | 0 / 0 |
| Settings | 10.634 ms | 10.484 ms | 58.65 | 0 / 0 |
| Boosted tilted Ink | 5.919–5.920 ms | 5.854–5.859 ms | 58.71–58.72 | 0 / 0 |

Ink composition remains approximately 7.6 ms; the change does not establish a substantial composition saving.
The recorded timing differences include code-placement effects and are not an isolated count of avoided polls.
A model test sweeps starting gates and actual/estimated periods from 14 to 20 ms.
It bounds wrap-detection delay to the previous polling interval plus a modeled 32-microsecond read cost.
The artwork, hardware-float and 24 KiB stack checks pass.
The final qualification repeats pixel comparisons and automatic-orientation workloads on the rebuilt image.
