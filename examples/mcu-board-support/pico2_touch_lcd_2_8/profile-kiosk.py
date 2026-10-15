#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import argparse
import json
import time
from pathlib import Path

import serial

import printer_protocol as printer


def main():
    parser = argparse.ArgumentParser(description="Measure steady animation inside the real ten-second Kiosk views")
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cycles", type=int, default=1)
    parser.add_argument("--require-panel-rate", action="store_true")
    args = parser.parse_args()
    if args.cycles < 1:
        parser.error("Cycles must be positive")
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    with serial.Serial(printer.find_port(args.serial), 115200, timeout=2, write_timeout=2) as port:
        try:
            printer.select_mode(port, "kiosk")
            started = time.monotonic()
            previous_window = printer.stats(port)["window"]
            for index in range(args.cycles * 3):
                deadline = started + 9.5 + index * 10
                time.sleep(max(0, deadline - time.monotonic()))
                values = printer.stats(port)
                if (values["window"] <= previous_window or values["mode"] != 11
                        or values["window_kiosk_stage"] != index % 3
                        or values["elapsed_us"] < 7_000_000
                        or values["kiosk_enabled"] != 1):
                    raise RuntimeError(f"Missing completed Kiosk scene window: {values}")
                previous_window = values["window"]
                values["workload"] = ["kiosk-queue", "kiosk-settings", "kiosk-ink"][index % 3]
                values["changing_fps"] = values["changing_frames"] * 1e6 / values["elapsed_us"]
                values["raster_ms_per_frame"] = values["raster_us"] / max(1, values["frames"]) / 1000
                results.append(values)
                (args.output / "measurements.json").write_text(json.dumps(results, indent=2) + "\n")
                print(f"{values['workload']}: {values['changing_fps']:.2f} FPS, "
                      f"{values['skipped_phases']} missed phases, "
                      f"{values['window_late_strips']} late strips, "
                      f"{values['stack_free']} unused stack bytes", flush=True)
                if values["cpu_hz"] != 150_000_000 or values["stack_free"] < 4096:
                    raise RuntimeError("Kiosk clock or stack qualification failed")
                if args.require_panel_rate and (
                    abs(values["changing_frames"] - values["window_panel_ticks"]) > 1
                    or values["skipped_phases"] != 0
                    or values["window_late_strips"] != 0
                    or values["window_scan_errors"] != 0
                ):
                    raise RuntimeError("Kiosk scene did not meet panel cadence")
        finally:
            printer.select_mode(port, "manual")


if __name__ == "__main__":
    main()
