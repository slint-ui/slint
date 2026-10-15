#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import argparse
import json
import time
from pathlib import Path

import serial
from printer_protocol import command, find_port, select_mode, stats

MODES = {
    "auto": 0,
    "pan-fast": 3,
    "poll-pan": 7,
    "settings-auto": 5,
    "settings-pan": 6,
    "settings-poll-pan": 8,
    "ink": 4,
    "ink-boost": 9,
    "ink-touch": 10,
    "ink-tilt": 12,
    "ink-tilt-boost": 13,
}
FAST_SET = ["auto", "settings-auto", "ink-boost"]
TIMES = [
    "raster", "wait", "window", "crc", "animation", "render_cpu", "dirty_cpu",
    "text_cpu", "texture_cpu", "rectangle_cpu", "rounded_cpu",
    "text_request", "text_font", "text_prepare", "text_submit",
    "ink_buffer", "ink_restore", "ink_stamp", "ink_publish", "plan",
]


def measure(port, mode, cpu_mhz):
    before = stats(port)
    started = time.monotonic()
    select_mode(port, mode)
    selected = stats(port)
    if selected["cpu_hz"] != cpu_mhz * 1_000_000:
        raise RuntimeError(f"Unexpected CPU clock: {selected['cpu_hz']}")
    expected_run = selected["current_run"]
    if expected_run & 255 != MODES[mode]:
        raise RuntimeError("Requested mode was replaced before measurement")
    time.sleep(12.05)
    deadline = time.monotonic() + 5
    while True:
        values = stats(port)
        if values["current_run"] != expected_run:
            raise RuntimeError("Another input replaced the requested benchmark run")
        if (values["window"] > before["window"]
                and values["profile_run"] == expected_run
                and values["elapsed_us"] >= 10_000_000
                and values["mode"] == MODES[mode]):
            break
        if time.monotonic() >= deadline:
            raise RuntimeError("No completed ten-second window for the requested run")
        time.sleep(.1)
    values["workload"] = mode
    values["mode_to_report_ms"] = (time.monotonic() - started) * 1000
    values["changing_fps"] = values["changing_frames"] * 1e6 / values["elapsed_us"]
    values["render_fps"] = values["frames"] * 1e6 / values["elapsed_us"]
    for name in TIMES:
        key = name + "_us"
        if key in values:
            values[key + "_per_frame"] = values[key] / max(1, values["frames"])
    if values["cpu_hz"] != cpu_mhz * 1_000_000:
        raise RuntimeError(f"Unexpected CPU clock: {values['cpu_hz']}")
    return values


def main():
    parser = argparse.ArgumentParser(description="Profile warmed native Pico printer workloads")
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", type=Path, required=True)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--modes", nargs="+", choices=MODES)
    selection.add_argument("--all", action="store_true")
    parser.add_argument("--cpu-mhz", type=int, choices=[150, 240], default=150)
    parser.add_argument("--require-panel-rate", action="store_true")
    parser.add_argument("--rotation", choices=["auto", "0", "1"], default="auto")
    args = parser.parse_args()
    modes = list(MODES) if args.all else args.modes or FAST_SET
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    with serial.Serial(find_port(args.serial), 115200, timeout=1, write_timeout=2) as port:
        try:
            if command(port, "rotation " + args.rotation) != "PICO_ROTATION_OK":
                raise RuntimeError("Unable to select display orientation")
            for mode in modes:
                values = measure(port, mode, args.cpu_mhz)
                values["rotation"] = args.rotation
                results.append(values)
                (args.output / "measurements.json").write_text(json.dumps(results, indent=2) + "\n")
                print(f"{mode}: {values['changing_fps']:.2f} changing FPS, "
                      f"{values['raster_us_per_frame'] / 1000:.3f} ms raster, "
                      f"{values['skipped_phases']} missed phases, "
                      f"{values['window_late_strips']} late strips, "
                      f"{values['window_scan_errors']} read errors", flush=True)
                if values["stack_free"] < 4096:
                    raise RuntimeError(f"{mode} has less than 4 KiB of measured main-stack headroom")
                if args.require_panel_rate and (
                    abs(values["changing_frames"] - values["window_panel_ticks"]) > 1
                    or values["skipped_phases"] != 0
                    or values["window_late_strips"] != 0
                    or values["window_scan_errors"] != 0
                ):
                    raise RuntimeError(f"{mode} did not meet panel cadence and zero-drop requirements")
        finally:
            command(port, "rotation auto")
            select_mode(port, "manual")


if __name__ == "__main__":
    main()
