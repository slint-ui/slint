# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import time
from serial.tools.list_ports import comports


def find_port(device_serial):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        ports = [
            p.device for p in comports()
            if p.vid == 0x2e8a and p.pid == 0x0009 and p.serial_number == device_serial
        ]
        if len(ports) == 1:
            return ports[0]
        if len(ports) > 1:
            raise RuntimeError("More than one port matches the requested serial")
        time.sleep(.25)
    raise RuntimeError("Requested Pico USB serial port is unavailable")


def command(port, text):
    port.reset_input_buffer()
    port.write((text + "\n").encode())
    response = port.readline()
    if not response.endswith(b"\n"):
        raise RuntimeError(f"Incomplete response to {text}: {response!r}")
    return response.decode().strip()


def select_mode(port, mode):
    response = command(port, "mode " + mode)
    if response != "PICO_MODE_OK":
        raise RuntimeError(response)


def stats(port):
    line = command(port, "stats")
    if not line.startswith("PICO_QUEUE,"):
        raise RuntimeError(line)
    values = dict(part.split("=", 1) for part in line.split(",")[1:])
    return {k: int(v) if v.lstrip("-").isdigit() else v for k, v in values.items()}
