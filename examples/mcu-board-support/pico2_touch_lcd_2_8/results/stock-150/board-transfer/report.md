# Pico Printer Board Improvements

The printer remains at 150 MHz CPU and 75 MHz flash.
Its native 320-by-240 UI, artwork, bubble sizes, populations, tilt, slosh and ripples are unchanged.
The retained image follows the physical panel at approximately 58.6 Hz rather than imposing an unrelated 60 Hz timer.

Guarded incremental flashing and fixed artwork placement are implemented first.
The short benchmark set, binary checks, pixel comparisons and stack guards also precede runtime tuning.
A later packet-memory reduction recovers room for the PIO transport without weakening the 24 KiB stack guard.

## Retained Changes

| Change | Measured Before and After | Practical Benefit |
| --- | --- | --- |
| Guarded changed-sector flashing | 8.462 s to 1.908 s for the controlled small change | 4.43 times faster development update, including preceding-image checks |
| Stable artwork placement | 24 named resources keep their bytes and addresses after a source rebuild | Executable growth does not force those resources to move; update savings still depend on the change |
| Event-driven packet ownership | Queue and Settings waiting each decrease about 0.1 ms per frame | Lower publication and slot-release latency |
| Display interrupt priority | Maximum observed transfer overhead decreases from 1.583 ms to 0.059 ms | Smaller display scheduling stalls; isolated Ink composition takes about 1.1 ms longer |
| Packet memory budget | 6 KiB recovered | More stack and transport space with unchanged full-height strip capacity |
| PIO display output | 48 MHz SPI to 60 MHz PIO; transfer busy time decreases about 20% | Approximately 2.1 ms less Ink transfer busy time per frame |
| CPU-scaled scan reads | Ink composition decreases from 7.951–7.979 ms to 7.621–7.663 ms | Less scan-polling work with unchanged beam margins |
| Once-bounded aligned packing | Queue 0.530 to 0.323 ms; Settings 0.732 to 0.436 ms; Ink 0.597 to 0.351 ms | 39–41% less packing time with identical bytes |
| Refresh-aware idle sampling | Queue raster 10.686 to 10.447 ms; Settings 10.634 to 10.484 ms in the isolated comparison | Fewer idle samples far from refresh; preserves the near-wrap polling bound |
| Capture-release acknowledgment | Wait for the UI core before accepting more capture/page commands | Closes a diagnostic protocol handoff gap |

Each row compares its own preceding image.
Transfer, composition, raster, planning and packing overlap or include waiting in different ways.
Do not add these savings together.
Code placement can change nearby timing buckets even when the local kernel is unchanged.

## Final Remaining Frame Budget

The panel interval is approximately 17.1 ms.
Subtract measured average animation and render/publish wall time to estimate the remaining interval.
Input and event-loop work are not fully attributed, so this is not guaranteed spare CPU time or a worst-case bound.

| Workload | Original Remaining Interval | Final Remaining Interval | Change |
| --- | ---: | ---: | ---: |
| Queue auto-scroll | 2.67 ms | 5.20 ms | +2.53 ms |
| Settings auto-scroll | 0.81 ms | 1.11 ms | +0.30 ms |
| Boosted tilted Ink | 3.02 ms | 2.32–2.39 ms | About -0.66 ms |

Ink has more display-link capacity and much smaller worst observed scheduling stalls, but less average CPU budget left.
The priority change retains this stability tradeoff for the zero-late-strip requirement.
It does not establish a net Ink CPU speedup.

All eight final timing windows match panel cadence with zero missed phases, late strips and scan errors.
The final sequence covers automatic rotation, flipped Ink and both touch-poll scrolling paths.
See the [full qualification](qualification/report.md) for timings, pixels, live flash checks, particle counts and kiosk evidence.

## Trials Left Out

- [Ink background block copies](ink-copy-rejected/report.md) show no consistent restoration benefit for the printer's short spans.
- [SRAM planner placement](planner-placement-rejected/report.md) saves 46–63 microseconds of planning but adds about 0.27 ms to Ink composition.
- [Bus-counter integration](bus-counters-rejected/report.md) is deferred after a diagnostic sequence triggers recovery.
  Later tests establish a separate capture handoff gap but do not prove it caused every earlier recovery.
  Counter code is absent from the qualified firmware.

## Other SDK and Native-Renderer Techniques

| Technique | Printer Decision |
| --- | --- |
| Automatic USB reset and watchdog BOOTSEL recovery | Already retained; no repeated button press is needed when runtime USB is available |
| 75 MHz flash, shorter chip-select gap and uninterrupted sequential bursts | Already retained and rechecked against the final ELF |
| Dedicated core 1 stack banks | Already retained; the stack remains 8 KiB and the main-core guard remains 24 KiB |
| Dual-core Ink composition | Already retained; UI objects remain on the Slint core and plain buffer work is joined before publication |
| DMA CRC sniffer | Already retained on its reserved channel |
| Cortex-M33 hard-float, ThinLTO and one code-generation unit | Retained; release disassembly checks hardware floating point and SRAM functions |
| Direct framebuffer and chained DMA | Not transferred: the printer reuses a mutable framebuffer, so DMA retains immutable packet ownership |
| Hardware-scroll transitions and backlight effects | Deferred because they change the visible UI behavior |
| Approximate SDK trigonometry and altered integer roots | Not substituted under the unchanged-visuals requirement; the printer preserves its existing math |
| Cache pinning, XIP streaming and cache-as-SRAM | Not enabled globally; native negative evidence and the printer's code-fetch needs argue against a blind port |
| Optional binary metadata or runtime startup configuration | Keep outside timing-qualified builds unless separately measured; existing image hashes identify these builds |
| Filesystem and PSRAM SDK support | No demonstrated printer throughput benefit; PSRAM support cannot add RAM absent from this board |

The retained changes are local to the Pico board feature and its development tools.
Other board backends and the native graphics checkout are unchanged.
See the [deletion audit](audit.md) for evidence cleanup and retained safety paths.
