# Event-Driven Packet Handoff

Packet publication wakes the display consumer, and releasing a packet wakes the producer.
Each waiter registers before checking ownership so a concurrent state change cannot lose its wakeup.
The idle display task retains a 500-microsecond scan-sampling timeout.
Immutable packet slots and round-robin ownership are unchanged.

| Workload | Previous Packet Wait | Event-Driven Packet Wait | Changing FPS | Missed Phases / Late Strips |
| --- | ---: | ---: | ---: | ---: |
| Queue | 2.066 ms | 1.959 ms | 58.76 | 0 / 0 |
| Settings | 3.994 ms | 3.881 ms | 58.72 | 0 / 0 |
| Boosted tilted Ink | 0.003 ms | 0.009 ms | 58.72 | 0 / 0 |

The repeated Ink window also passes with zero scan errors.
Queue and Settings each avoid approximately 0.1 ms of packet-slot waiting per frame.
Ink has little packet-space pressure and gains no measurable rendering benefit from this change.
Raster times are 11.016 ms, 10.547 ms and 5.750–5.752 ms respectively, slightly above the preceding layout build.
This change targets handoff latency rather than raster work.
All four captured Queue and Settings frames are byte-identical to the layout baseline, and both panel-memory checks match all 76,800 pixels.
The code-generation and artwork checks pass with the 24 KiB stack guard intact.
