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

Hardware floating point reduces rendering time by 0.5%, 1.5%, and 2.0%, respectively.
The LCD transfer remains about 37 ms.

Speed optimization increases the application from 2,149,232 to 2,635,184 bytes, within the 16 MB flash.
The compiler package retains its optimization-level override of 2.

Speed optimization reduces rendering time by another 10.1%, 20.5%, and 29.2%, respectively.
