#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Record physical CST328 reports without injecting pointer events."""

import argparse
import json
import time
from pathlib import Path

import serial
from printer_protocol import command, find_port


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=60)
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    args.output.mkdir(parents=True, exist_ok=True)
    rates, reports, seen = [], [], set()
    with serial.Serial(find_port(args.serial), 115200, timeout=2, write_timeout=2) as port:
        started = time.monotonic()
        previous = None
        while time.monotonic() - started < args.seconds:
            now = time.monotonic()
            line = command(port, "touchstats")
            if not line.startswith("PICO_TOUCH,"):
                raise RuntimeError(line)
            values = {k: int(v) for k, v in (part.split("=") for part in line.split(",")[1:])}
            row = {"seconds": now - started, **values}
            if previous is not None:
                dt = now - previous[0]
                row["rates"] = {
                    k: ((v - previous[1][k]) % 2**32) / dt
                    for k, v in values.items() if k != "max_gap_us"
                }
                print(f"{row['seconds']:.1f}s: "
                      f"{row['rates']['contacts']:.1f} contacts/s, "
                      f"{row['rates']['changed']:.1f} changed/s, "
                      f"{row['rates']['releases']:.1f} releases/s", flush=True)
            rates.append(row)
            previous = now, values
            line = command(port, "touchtrace")
            if not line.startswith("PICO_TOUCH_TRACE"):
                raise RuntimeError(line)
            for item in line.split(",")[1:]:
                sequence, timestamp, first, last = item.split(":")
                sequence = int(sequence)
                if sequence == 0 or sequence in seen:
                    continue
                seen.add(sequence)
                data = int(first, 16).to_bytes(4, "little") + int(last, 16).to_bytes(2, "little")
                reports.append({"sequence": sequence, "time_us": int(timestamp),
                                "raw": data.hex(), "status": data[0] & 15,
                                "fingers": data[5] & 15})
            for name, data in [("rates", rates), ("reports", reports)]:
                (args.output / (name + ".json")).write_text(json.dumps(data, indent=2) + "\n")
            time.sleep(.1)


if __name__ == "__main__":
    main()
