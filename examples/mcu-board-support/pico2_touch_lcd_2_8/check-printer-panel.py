#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import argparse
import json
import time
from pathlib import Path

import serial
from printer_protocol import command, find_port, select_mode


def main():
    parser = argparse.ArgumentParser(description="Compare the printer framebuffer with panel GRAM")
    parser.add_argument("--serial", required=True)
    parser.add_argument("--modes", nargs="+", default=["auto", "settings-auto", "ink"],
                        choices=["auto", "settings-auto", "ink", "ink-boost"])
    parser.add_argument("--offsets", nargs="+", type=int, default=[12, 13, 14])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if any(not 0 <= offset <= 207 for offset in args.offsets):
        parser.error("Offsets must be between zero and 207 pixels")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    results = []
    with serial.Serial(find_port(args.serial), 115200, timeout=30, write_timeout=2) as port:
        try:
            for mode in args.modes:
                select_mode(port, mode)
                time.sleep(2.25)
                for offset in args.offsets:
                    started = time.monotonic()
                    response = command(port, f"panelcheck {offset}")
                    fields = response.split(",")
                    if len(fields) != 6 or fields[0] != "PICO_PANEL_CHECK":
                        raise RuntimeError(response)
                    result = dict(zip(
                        ["checked_pixels", "mismatches", "first_index", "expected", "actual"],
                        map(int, fields[1:]),
                    ))
                    result.update(workload=mode, offset=offset,
                                  elapsed_seconds=time.monotonic() - started)
                    results.append(result)
                    args.output.write_text(json.dumps(results, indent=2) + "\n")
                    print(f"{mode}, offset {offset}: {result['mismatches']} mismatches "
                          f"in {result['checked_pixels']} pixels", flush=True)
                    if result["checked_pixels"] != 76800 or result["mismatches"]:
                        raise RuntimeError("Panel memory differs from the framebuffer")
        finally:
            select_mode(port, "manual")


if __name__ == "__main__":
    main()
