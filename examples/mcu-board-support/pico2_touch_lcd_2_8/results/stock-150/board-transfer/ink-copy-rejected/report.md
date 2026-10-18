# Rejected Ink Background Block Copy

The trial copies aligned RGB565 background spans with an SRAM-resident LDM/STM loop.
It handles an initial halfword and a final tail without changing pixels.
All four timing windows meet panel cadence with zero missed phases, late strips and scan errors.
The temporary alignment and guard test passes alongside the existing host suite: 43 tests in total.

However, boosted tilted Ink restoration changes from 2.821–2.872 ms to 2.841–2.905 ms.
Composition changes from 7.573–7.600 ms to 7.540–7.577 ms, too small and inconsistent with restoration timings to establish a benefit.
The printer's many short, partially aligned spans differ from the native renderer's long frame copies.
The copy implementation and its temporary test are removed.
The existing safe slice copies remain in production source.
