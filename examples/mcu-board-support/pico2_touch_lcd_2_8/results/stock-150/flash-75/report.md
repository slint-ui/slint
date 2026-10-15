# 75 MHz Flash at Stock CPU Speed

<!-- cSpell:ignore flashhash objcopy Winbond -->

The stock 150 MHz firmware selects a flash divisor of 2 after clock initialization and watchdog arming.
The change applies only to the measured `0xEB` quad-I/O read mode.
Basic serial-read fallback keeps its existing timing.
CPU frequency remains 150 MHz, display SPI remains 48 MHz, and output remains native 320 × 240.
Read command, bus width, dummy cycles, and receive delay remain unchanged.
The measured receive delay is 2 half-system-clock cycles.

## Controlled Comparison

One profiling binary alternates 50, 75, 50, and 75 MHz, keeping code placement identical.
Each clock pass measures Queue, Settings, and boosted tilted Ink.
Each workload settles for two seconds and retains ten seconds.
Rotation is locked while measuring continuous motion; full-screen flips are outside these windows.
The table averages the two retained windows at each clock.

| Stage | 50 MHz Flash | 75 MHz Flash | Time Reduction |
| --- | ---: | ---: | ---: |
| Queue Slint raster | 11.802 ms | 10.559 ms | 10.5% |
| Settings Slint raster | 11.786 ms | 10.242 ms | 13.1% |
| Boosted tilted Ink Slint raster | 7.460 ms | 6.461 ms | 13.4% |
| Boosted tilted Ink vial composition | 5.653 ms | 5.279 ms | 6.6% |

All twelve [comparison windows](comparison.json) match panel cadence with zero missed phases, late strips, and scan errors.
Changing-frame rates span 58.54–58.65 FPS.
Normal rendering, particle counts, sizes, tilt, and artwork are unchanged.

Queue's total measured draw path changes from 14.330 to 14.460 ms; Settings changes from 16.107 to 16.302 ms.
Queue packet waiting increases from 1.020 to 2.454 ms, and Settings from 2.415 to 4.220 ms.
Earlier raster completion exposes more waiting for free display packets.
The result saves raster work but does not increase presentation throughput for these panel-limited workloads.
Boosted tilted Ink saves about 1.37 ms across vial composition and Slint raster.
These stages exclude other event-loop work and do not measure physical input latency.

## Final Startup Build

The temporary runtime clock selector is removed from the retained firmware.
A fresh flash and reboot confirms that startup itself selects 75 MHz.
The final binary has a different layout, so its measurements are qualified separately from the isolated clock comparison.

| Workload | Changing FPS | Slint Raster | Vial Composition | Missed Phases | Late Strips | Scan Errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Queue | 58.56 | 10.230 ms | 0.000 ms | 0 | 0 | 0 |
| Settings | 58.54 | 10.264 ms | 0.000 ms | 0 | 0 | 0 |
| Boosted tilted Ink | 58.55 | 6.401 ms | 5.299 ms | 0 | 0 | 0 |

[Final qualification](qualification.json) retains the live clock report, completed-frame counters, panel checks, and flash integrity results.
The device remains on Ink with live sensor input and automatic orientation enabled.

## Integrity and Regression Checks

- Eight uncached flash-read passes in the comparison match host-computed FNV-1a hashes for all five loaded sections, totaling 1,805,884 bytes per pass.
- Three further passes on the final startup build match across 1,805,780 bytes per pass.
  The checks cover vectors, boot block, code, read-only artwork and fonts, and the SRAM initialization image.
- Eighteen controller-memory checks match all 76,800 framebuffer pixels each.
  The final six checks cover Queue, Settings, and boosted tilted Ink in both hardware orientations.
- Twenty frozen Queue and Settings captures preserve identical pixels at offsets 12 and 13 across clock configurations and the final build.
  Every capture passes hardware-to-host CRC validation.
- The native Queue, Settings, and tilted Ink captures were inspected without visible framebuffer corruption.
- All 35 release host tests pass, and normal and profiling release configurations build successfully.
- Disassembly confirms that the guarded divider update executes from SRAM and preserves the other timing bits.
  The watchdog is armed before tuning, and the update precedes starting core 1.

Flash checks use volatile reads through the alias at `0x14000000`, bypassing the cache without filling it.
They run outside timing windows and yield to the display executor every KiB.
The retained `flashhash` command and `check-printer-flash.py` reproduce the integrity checks without changing flash timing.
Integrity comparisons use 32-bit FNV-1a hashes.
Controller-memory parity and framebuffer captures do not optically measure the live panel scan.
These runs qualify the attached board at its observed conditions; voltage and temperature margins were not swept.

## Reproduce Final Checks

Build the profiling release with the existing stock-speed script, then run:

```sh
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/profile-printer.py --serial <serial> --modes auto settings-auto ink-tilt-boost --rotation 0 --require-panel-rate --output /tmp/pico-flash75
python3 examples/mcu-board-support/pico2_touch_lcd_2_8/check-printer-flash.py --serial <serial> --elf <flashed.elf> --repeats 3 --output /tmp/pico-flash75-integrity.json
```

The flash checker requires LLVM `objdump` and `objcopy` on PATH or their paths supplied with `--objdump` and `--objcopy`.
[Firmware metadata](firmware.json) records the tested binaries and memory headroom.
The clock-divisor and receive-delay behavior follow the [RP2350 QMI register specification](https://github.com/raspberrypi/pico-sdk/blob/master/src/rp2350/hardware_regs/include/hardware/regs/qmi.h).
The flash read-mode limits follow the [Winbond W25Q128JV data sheet](https://www.winbond.com/resource-files/w25q128jv%20revg%2004082019%20plus.pdf), sections 9.2 and 9.6.
