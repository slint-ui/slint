# Pico 2 Printer Performance

The Waveshare Pico2 Touch LCD 2.8 runs the printer UI at native 320 × 240 resolution.
Slint owns layout, controls, pointer handling, scrolling, text, and software rasterization.
Custom Rust composes the four Ink vial images, including their glass, liquid, and particles.
Slint displays those images and draws the surrounding UI.

![Native Ink framebuffer captured from the final firmware](ink-particles.png)

Use the [stock-speed qualification](stock-150/report.md) for current measurements and limitations.
The retained results contain final qualification data and native pixel references.
See the [profiling guide](../PROFILING.md) for measured hot spots, five further experiments, and deeper tools.

## Reproduce Measurements

Build and flash a release profiling image:

```sh
PICO_PROFILE=1 PICO_SERIAL=<serial> examples/mcu-board-support/pico2_touch_lcd_2_8/flash-printer.sh
```

Install Python's `pyserial` package, then run the input workloads:

```sh
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/profile-printer.py --serial <serial> --all --require-panel-rate --output /tmp/pico-input
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/profile-kiosk.py --serial <serial> --cycles 2 --require-panel-rate --output /tmp/pico-kiosk
```

Each input workload settles for two active seconds before retaining ten seconds.
Kiosk changes pages every ten seconds, retaining seven seconds after two seconds of settling within each page.
The strict checks reject missed animation phases, late display strips, scan errors, unexpected clocks, and insufficient measured stack headroom.
They allow one frame at the measurement boundary when comparing completed changing frames with panel ticks.
Synthetic pointer replay measures dispatch and rendering throughput, not physical touch-sensor latency.

Compare all 76,800 controller pixels with the framebuffer:

```sh
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/check-printer-panel.py --serial <serial> --offsets 12 13 14 --output /tmp/pico-panel.json
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/capture-printer.py /tmp/pico-capture --serial <serial> --offsets 13
```

Captures include a hardware-computed CRC checked by the host.
Panel checks wait for partial transfers without replacing them with a full repaint.
Neither counters nor controller memory prove that the physical scan is visually clean.
The `reference` directory contains Queue and Settings frames at adjacent whole-pixel offsets for regression comparisons.

## Retained Optimizations

- Run Slint on core 0 and scan-synchronized SPI DMA and USB on core 1.
- Use hardware rotation and a native big-endian RGB565 framebuffer.
- Keep three immutable 32 KiB packet slots until DMA completes.
- Clock the stock CPU at 150 MHz and the peripheral PLL at 96 MHz, providing 48 MHz SPI independently.
  USB and ADC remain at 48 MHz.
- Stream pixel payloads as 16-bit SPI words with DMA byte swapping.
  Commands and scan reads retain their separate 8-bit configuration.
- Select a scan wave using the actual ordered rectangle costs, including beam waits, command allowance, and admission margin.
- Compare exact uniform 8 × 8 tiles and omit only tiles whose retained color is unchanged.
  Nonuniform tiles are always uploaded; this optimization does not rely on a lossy hash.
- Place measured hot dispatch, traversal, texture, border, text, and display routines in executable SRAM.
- Cache prepared bitmap paragraphs with bounded entry and glyph counts.
- Reuse tracked item geometry independently of appearance changes.
- Normalize dirty rectangles once per frame, then intersect primitives and glyphs with those rectangles.
- Copy opaque RGB565 spans and fill opaque rows with native word operations.
- Preserve fractional glyph bearings while specializing native-size bitmap arithmetic.
- Restore the union of previous Ink footprints and borrow static vial artwork instead of retaining composed backgrounds.

Pixel SPI uses the continuous Motorola mode documented in the [RP2350 data sheet](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf), sections 12.3.4.10–12.3.4.13.
The measured clock and scan allowances apply to this board and firmware; other panels require qualification.

## Memory and Recovery

The linker reserves at least 20 KiB for the main stack and places the 8 KiB core-1 stack in a separate SRAM bank.
The heap has 316 KiB; link-time assertions reject overlaps.
The profiling image measures unused main-stack space with a startup marker.

At stock speed, a watchdog reset from either stalled core returns to ROM BOOTSEL.
Core 1 reloads the eight-second watchdog only while core 0 has a heartbeat younger than four seconds.
A core-0 stall therefore takes up to roughly twelve seconds to recover; a core-1 stall takes roughly eight seconds.
The watchdog starts after clock initialization and cannot recover faults before it is armed.
Intentional USB reset requests clear the marker before rebooting.
The optional 240 MHz build retains its fallback to stock clock after a watchdog reset.

Normal Ink uses 16/26/20/32 particles at either clock.
Both clock configurations enable held-vial boosts, preserving extra bubbles until they leave the liquid surface.
The [boost qualification](stock-150/boost-qualification/report.md) records the stock-speed optimization and final regression checks.
