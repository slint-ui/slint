# Flash the Printer Demo on ESP32-S3-BOX-3

Use the combined `printerdemo-box3.bin` image for an ESP32-S3-BOX-3 with 16 MB flash.
This image targets the native 320×240 display and GT911 touch controller.
It is not compatible with BOX, BOX-Lite, or the Makerfabs round display.

The image includes an ESP-IDF bootloader, partition table, and release printer application.
Flash the complete image at address `0x0`, including on a factory device.
Flashing replaces the existing firmware and initializes the NVS and PHY storage areas.
No device credentials or copied calibration data are included.

## Flash from macOS, Linux, or Windows

Connect the BOX-3 directly by USB and close programs using its serial port.
Install esptool 5.5.0 in a virtual environment:

```sh
python3 -m venv .flash-tools
```

Activate it on macOS or Linux:

```sh
. .flash-tools/bin/activate
```

On Windows PowerShell, activate it with `.flash-tools\Scripts\Activate.ps1` instead.
Use `python` instead of `python3` if that is your Python command.

```sh
python -m pip install esptool==5.5.0
python -m serial.tools.list_ports
```

Choose the BOX-3 serial port from the output.
Examples are `/dev/cu.usbmodem1101` on macOS, `/dev/ttyACM0` on Linux, and `COM5` on Windows.
From the extracted firmware directory, replace `PORT` in this command:

```sh
python -m esptool --chip esp32s3 --port PORT --baud 921600 \
  write-flash 0x0 printerdemo-box3.bin
```

The flasher verifies the written data and resets the device.
If automatic connection fails, hold Boot, press Reset, release Boot, and retry.
If transfer errors occur, retry with `--baud 115200`.

## Verify and Use the Demo

Compare the firmware SHA-256 against `SHA256SUMS` or `manifest.json` before flashing.
On macOS or Linux, run `shasum -a 256 printerdemo-box3.bin`.
On Windows, run `Get-FileHash printerdemo-box3.bin -Algorithm SHA256`.

The demo opens on the printer queue.
Drag the queue or Settings list to scroll, including flick inertia.
Use the left navigation buttons to open Ink and its bubbling vials.
The application is a local simulation and requires no Wi-Fi connection.

The release image has no automated benchmark gestures or profiling capture output.
The manifest identifies the source commit, build features, compiler flags, bootloader, and component hashes.
See [the build instructions](README.md#qualified-esp32-s3-box-3-build) to build the application from source.
The accompanying source archive includes the modified Slint source and its license files.
