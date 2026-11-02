# BOX-3 Performance

These measurements use the attached ESP32-S3-BOX-3 at 240 MHz, with a 40 MHz LCD SPI clock.
The early renderer and DMA comparison pairs used backlight blanking in both variants.
Only their rendering and transfer measurements are retained here.
Each optimization has its own commit and uses the preceding version as its baseline.

## Method

Release firmware enables `mcu-board-support/box-display-profile` during measurement.
`BOX_FRAME` measures rendering and packet preparation, ending when the display worker accepts the frame.
`BOX_TRANSFER` measures the pixel transfer.
These intervals are separate measurements; USB upload acknowledgments don't measure when the new image becomes visible.

The scenes are a solid background, the connected counter demo, and a clipping/rotation/gradient/text scene.
Each scene uses the same saved source across firmware versions.
The counter and clipping scenes receive six successive designs through the editor's USB protocol.
The solid background alternates colors every 500 ms on the device.
The table reports median rendering and packet preparation times, excluding the initial paint.
There are five samples per design scene and seventeen samples for the solid background.
Pixel checksums cover the packed RGB565 bytes sent to the LCD.
Startup screens containing variable memory statistics are excluded from pixel comparisons.

## Results

| Change | Solid background | Counter screen | Clipping and gradients | Pixel comparison |
| --- | ---: | ---: | ---: | --- |
| Baseline: software floating point, size optimization | 15.120 ms | 70.739 ms | 81.204 ms | Reference |
| Hardware floating point | 15.045 ms | 69.681 ms | 79.584 ms | All scene checksums match |
| Speed optimization (`opt-level = 3`) | 13.518 ms | 55.392 ms | 56.339 ms | All scene checksums match |
| Accelerated background spans | 11.916 ms | 53.824 ms | 54.701 ms | All scene checksums match |
| Direct native-size glyph sampling | 11.835 ms | 53.309 ms | 54.160 ms | All scene checksums match |

Hardware floating point reduces rendering time by 0.5%, 1.5%, and 2.0%, respectively.
The LCD transfer remains about 37 ms.

Speed optimization increases the application from 2,149,232 to 2,635,184 bytes, within the 16 MB flash.
The compiler package retains its optimization-level override of 2.

Speed optimization reduces rendering time by another 10.1%, 20.5%, and 29.2%, respectively.

Accelerated background spans reduce rendering time by another 11.9%, 2.8%, 2.9%, respectively.
The optimization applies to both full-buffer and line rendering.
Opaque backgrounds use the target pixel span operation, enabling the BOX PIE fill routine.
Translucent and gradient backgrounds retain their alpha behavior.

Direct glyph sampling reduces the counter and clipping medians by about 1%.
The solid-fill measurement also varies by 0.7%, although it contains no text.
A separate text-heavy scene checks that the glyph improvement extends beyond that variation.
It contains sixteen rows of 12-pixel text, uploaded through the same USB replacement sequence.
Its median falls from 93.517 ms to 89.259 ms, a 4.6% reduction, with matching pixel checksums.
The scalar-sampling test covers clipping, tiling, fractional offsets, opacity, and four target pixel formats.

## DMA Copy and Transfer Overlap at 40 MHz

The reference uses one 16 KiB DMA transmit buffer.
It copies each chunk before starting its transfer and waits before preparing the next one.
The pipeline uses two 8 KiB buffers, keeping the transmit-data allocation at 16 KiB.
The DMA transfer owns its active buffer while the CPU fills the other one.
The SPI clock and transmitted pixels stay fixed.

`BOX_DMA` reports CPU copying, DMA setup, remaining blocking wait, chunk count, and copying performed during an active transfer.
The profile counters are disabled in normal firmware.
The full-screen payload remains 153,600 bytes, whose wire time at 40 MHz is 30.720 ms.

| Scene | Reference render | Pipelined render | Reference transfer | Pipelined transfer |
| --- | ---: | ---: | ---: | ---: |
| Solid background | 12.027 ms | 11.946 ms | 37.279 ms | 32.413 ms |
| Counter | 52.881 ms | 52.364 ms | 37.367 ms | 32.597 ms |
| Clipping and gradients | 55.778 ms | 55.655 ms | 37.371 ms | 32.577 ms |
| 80 × 60 partial update | 3.686 ms | 3.619 ms | 2.480 ms | 2.632 ms |

Solid-background copying takes about 5.5 ms in both versions.
The pipeline hides about 5 ms of that work behind the active transfer.
The remaining DMA wait falls from about 31.1 ms to 26.1 ms.
Full-screen transfer falls by about 4.9 ms, or 13%.

The reference solid-background run contains seventeen steady samples; the pipelined run contains sixteen.
The counter and clipping comparisons each contain five replacements after initial paint.
The partial scene alternates an 80 × 60 rectangle every 500 ms, crossing the 8 KiB buffer boundary.
The two-buffer version makes this partial transfer about 0.15 ms slower.
There are fifteen steady partial samples in the reference and seventeen in the pipelined version.
Partial updates keep the backlight on.
Packed-pixel hashes match for all scenes.
Host tests exercise the actual DMA adapter with deferred buffer consumption, partial tails, repeated reuse, submission failures, and retry.

The retained change only overlaps copying and DMA; it does not eliminate the copied bytes.
The full-screen transfer is now about 1.7 ms above the 40 MHz pixel wire time.

The final firmware runs at 40 MHz with profiling disabled and USB live preview connected.
The user confirmed normal display geometry, clean background color changes, and working touch on the attached BOX-3.

## 80 MHz PSRAM

This comparison keeps the LCD at 40 MHz and flash in DIO mode.
Only the octal PSRAM clock changes from 40 MHz to 80 MHz.
The reference includes the accepted renderer and DMA optimizations.

| Scene | 40 MHz render | 80 MHz render | 40 MHz transfer | 80 MHz transfer |
| --- | ---: | ---: | ---: | ---: |
| Solid background | 11.979 ms | 8.342 ms | 32.407 ms | 32.026 ms |
| Counter | 52.674 ms | 47.102 ms | 32.553 ms | 32.132 ms |
| Clipping and gradients | 54.697 ms | 49.266 ms | 32.559 ms | 32.128 ms |
| Sixteen text rows | 86.753 ms | 79.893 ms | 32.511 ms | 32.159 ms |
| 80 × 60 partial update | 3.593 ms | 3.540 ms | 2.599 ms | 2.587 ms |

Rendering falls by 30.4% for the solid background and 7.9–10.6% for the other full-screen scenes.
Full-screen transfer falls by about 0.4 ms; its pixel wire time remains 30.720 ms.
All five scenes produce matching pixel checksums.
Source replacements use five steady samples; timer scenes use up to seventeen.
The profile build takes 10.63 seconds with a warm build cache.

## QIO Flash

The same 80 MHz PSRAM application is flashed with QIO instead of the runner's default DIO mode.
The LCD stays at 40 MHz.
The explicit flash mode selects espflash's QIO bootloader.

| Scene | DIO render | QIO render | DIO transfer | QIO transfer |
| --- | ---: | ---: | ---: | ---: |
| Solid background | 8.342 ms | 7.507 ms | 32.026 ms | 31.735 ms |
| Counter | 47.102 ms | 35.187 ms | 32.132 ms | 31.853 ms |
| Clipping and gradients | 49.266 ms | 39.009 ms | 32.128 ms | 31.823 ms |
| Sixteen text rows | 79.893 ms | 66.080 ms | 32.159 ms | 31.814 ms |
| 80 × 60 partial update | 3.540 ms | 2.468 ms | 2.587 ms | 2.334 ms |

QIO reduces rendering by 10.0–30.3% across these scenes.
All five scenes retain their pixel checksums and complete USB replacement or timer-update checks.
The flash and PSRAM settings match [CircuitPython's BOX-3 configuration](https://github.com/adafruit/circuitpython/blob/main/ports/espressif/boards/espressif_esp32s3_box_3/mpconfigboard.mk).

## Remote Edit Delay

Serial preview coalesces changes for 16 ms instead of 50 ms before compiling.
The desktop preview keeps its existing delay.
This removes 34 ms of intentional waiting per settled edit.
The firmware remains the QIO, 80 MHz PSRAM build with a 40 MHz LCD.

| Scene | 50 ms delay: edit to loaded | 16 ms delay: edit to loaded | 50 ms delay: edit to write completion | 16 ms delay: edit to write completion |
| --- | ---: | ---: | ---: | ---: |
| Counter | 260.024 ms | 199.356 ms | 332.580 ms | 270.543 ms |
| Clipping and gradients | 180.346 ms | 153.085 ms | 260.354 ms | 239.297 ms |
| Sixteen text rows | 174.942 ms | 148.221 ms | 279.493 ms | 249.316 ms |

These five-replacement medians include compilation, USB scheduling, and device work.
They vary with host load and panel phase; their differences don't isolate the fixed 34 ms saving.
Write completion means receipt of the device's final-SPI-byte log, rather than physical panel scanout.
Rendering and transfer measurements remain within normal variation, and all scene hashes match.
All six serial transport tests pass in release mode.

## USB and Input Servicing Between Display Ticks

The application services timers and touch before waiting for the next display tick.
The viewer's 5 ms USB timer can run between ticks, while rendering retains its existing panel gate and recovery policy.
Both measurements use the same font-snapshot host probe and QIO, 80 MHz PSRAM firmware.

| Scene | Tick-gated receive/load | Independent receive/load | Previous render | New render | Previous transfer | New transfer |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Counter | 59 ms | 47 ms | 35.164 ms | 35.194 ms | 31.852 ms | 31.613 ms |
| Clipping and gradients | 33 ms | 24 ms | 39.643 ms | 38.737 ms | 31.825 ms | 31.631 ms |
| Sixteen text rows | 29 ms | 19 ms | 65.929 ms | 64.695 ms | 31.827 ms | 31.590 ms |

Receive/load is the host transport's measured interval from sending the compiled design to the device's load acknowledgment.
It excludes desktop compilation and uses five steady replacements.
Initial counter upload is nearly unchanged, at 278 ms versus 271 ms; this change doesn't remove all upload overhead.
No touch-latency measurement is claimed.
All five scenes retain their pixel hashes, and all 41 frame-queue host tests pass.
