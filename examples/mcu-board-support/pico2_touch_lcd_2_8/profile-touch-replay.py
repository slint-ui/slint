#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Replay captured physical drags through the cross-core sample buffer."""

import argparse
import csv
import json
import statistics
import time
from pathlib import Path

import serial
from printer_protocol import command, find_port, select_mode, stats

TRACE = Path(__file__).parent / "results/stock-150/touch-panning/panel-history/reports.csv"
PAGES = [("Queue", "auto", 11, 668), ("Settings", "settings-auto", 684, 1484)]


def collect(port, events):
    line = command(port, "touchdispatch")
    if not line.startswith("PICO_TOUCH_DISPATCH"):
        raise RuntimeError(line)
    for value in line.split(",")[1:]:
        sequence, timestamp, sampled, meta = map(int, value.split(":"))
        if sequence:
            events[sequence] = dict(sequence=sequence, time_us=timestamp, sample_us=sampled,
                                    kind=meta >> 16, history_count=meta & 65535)


def distribution(values):
    ordered = sorted(values)
    return dict(count=len(values), min=min(values), median=statistics.median(values),
                mean=statistics.mean(values), p95=ordered[int(.95 * (len(values) - 1))],
                maximum=max(values))


def replay(port, page, rows, events):
    collect(port, events)
    first = max(events, default=0) + 1
    duration = (int(rows[-1]["time_us"]) - int(rows[0]["time_us"])) / 1e6
    if command(port, "touchreplay " + page.lower()) != "PICO_TOUCH_REPLAY_OK":
        raise RuntimeError("Unable to start device-timed replay")
    deadline = time.monotonic() + duration + .3
    while time.monotonic() < deadline:
        collect(port, events)
        time.sleep(.025)
    delivered = [events[n] for n in sorted(events) if n >= first]
    holes = [[a["sequence"], b["sequence"]] for a, b in zip(delivered, delivered[1:])
             if b["sequence"] != a["sequence"] + 1]
    if holes:
        raise RuntimeError(f"Incomplete dispatch trace: {holes}")
    moves = [e for e in delivered if e["kind"] == 1]
    gaps = [(b["time_us"] - a["time_us"]) / 1000 for a, b in zip(moves, moves[1:])]
    contacts = sum(int(r["status"]) == 6 for r in rows)
    accounted = sum(e["kind"] != 2 for e in delivered) + sum(e["history_count"] for e in moves)
    if contacts != accounted:
        raise RuntimeError(f"Coordinate loss: {accounted} delivered from {contacts} supplied")
    if sum(e["kind"] == 0 for e in delivered) != 1 or sum(e["kind"] == 2 for e in delivered) != 1:
        raise RuntimeError("Replay did not preserve one continuous contact")
    return dict(sequence_first=first, sequence_last=max(events), movement_events=len(moves),
                acquisition_coordinates=contacts, coordinates_accounted_for=accounted,
                gaps_ms=distribution(gaps), gaps_over_20_ms=sum(g > 20 for g in gaps),
                sample_age_ms=distribution([(e["time_us"] - e["sample_us"]) / 1000 for e in moves]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    args.output.mkdir(parents=True, exist_ok=True)
    with TRACE.open() as source:
        reports = list(csv.DictReader(source))
    events, runs = {}, []
    with serial.Serial(find_port(args.serial), 115200, timeout=2, write_timeout=2) as port:
        try:
            command(port, "rotation 0")
            for repeat in range(args.repeats):
                for page, mode, lo, hi in PAGES:
                    select_mode(port, mode)
                    time.sleep(1)
                    select_mode(port, "manual")
                    time.sleep(.3)
                    before = stats(port)
                    rows = [r for r in reports if lo <= int(r["sequence"]) <= hi]
                    result = dict(page=page, repeat=repeat, **replay(port, page, rows, events))
                    after = stats(port)
                    result["late_strips"] = after["late_strips"] - before["late_strips"]
                    result["scan_errors"] = after["scan_errors"] - before["scan_errors"]
                    result["touch_counters"] = command(port, "touchstats")
                    runs.append(result)
                    for name, values in [("runs", runs), ("events", list(events.values()))]:
                        (args.output / (name + ".json")).write_text(json.dumps(values, indent=2) + "\n")
                    print(json.dumps(result), flush=True)
                    if after["cpu_hz"] != 150_000_000 or result["late_strips"] or result["scan_errors"]:
                        raise RuntimeError("Replay failed clock or display-safety qualification")
        finally:
            select_mode(port, "manual")
            command(port, "rotation auto")


if __name__ == "__main__":
    main()
