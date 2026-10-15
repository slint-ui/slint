#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import argparse
import hashlib
import os
from pathlib import Path
import struct
import zlib

import serial
from printer_protocol import command, find_port


def png_chunk(kind, data):
    return (struct.pack("!I", len(data)) + kind + data
            + struct.pack("!I", zlib.crc32(kind + data)))


def png(data):
    rgb = bytearray()
    for (pixel,) in struct.iter_unpack(">H", data):
        red, green, blue = pixel >> 11, (pixel >> 5) & 63, pixel & 31
        rgb.extend(((red << 3) | (red >> 2), (green << 2) | (green >> 4),
                    (blue << 3) | (blue >> 2)))
    rows = b"".join(b"\0" + rgb[y * 960:(y + 1) * 960] for y in range(240))
    return (b"\x89PNG\r\n\x1a\n"
            + png_chunk(b"IHDR", struct.pack("!2I5B", 320, 240, 8, 2, 0, 0, 0))
            + png_chunk(b"IDAT", zlib.compress(rows)) + png_chunk(b"IEND", b""))


def main():
    parser = argparse.ArgumentParser(description="Capture native RGB565 printer frames")
    parser.add_argument("destination", type=Path)
    parser.add_argument("--offsets", default="12,13,14")
    parser.add_argument("--serial", default=os.environ.get("PICO_SERIAL"), required=not os.environ.get("PICO_SERIAL"))
    args = parser.parse_args()
    offsets = [int(offset) for offset in args.offsets.split(",")]
    if any(not 0 <= offset <= 207 for offset in offsets):
        parser.error("Offsets must be between zero and 207 pixels")
    args.destination.mkdir(parents=True, exist_ok=True)
    with serial.Serial(find_port(args.serial), 115200, timeout=30, write_timeout=5) as port:
        for offset in offsets:
            header = command(port, f"capture {offset}")
            if not header.startswith("PICO_FRAME,320,240,153600,"):
                raise RuntimeError(f"Invalid frame header: {header}")
            fields = dict(part.split("=", 1) for part in header.split(",")[4:])
            data = port.read(153600)
            if len(data) != 153600 or fields["format"] != "be":
                raise RuntimeError("Incomplete or unsupported framebuffer capture")
            if zlib.crc32(data) != int(fields["crc32"]):
                raise RuntimeError("Hardware frame CRC does not match received pixels")
            (args.destination / f"queue-{offset}.rgb565").write_bytes(data)
            (args.destination / f"queue-{offset}.png").write_bytes(png(data))
            print(offset, hashlib.sha256(data).hexdigest(), flush=True)


if __name__ == "__main__":
    main()
