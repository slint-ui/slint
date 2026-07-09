#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import argparse
import csv
import math
from pathlib import Path


SOURCES = ("slint,", "android-over-scroller,")


def csv_payload(line):
    for source in SOURCES:
        index = line.find(source)
        if index >= 0:
            return line[index:]
    return None


def read_trace(path, gesture):
    rows = []
    for raw_line in Path(path).read_text(encoding="utf-8").splitlines():
        payload = csv_payload(raw_line)
        if payload is None:
            continue
        fields = next(csv.reader([payload]))
        if len(fields) != 7:
            continue
        source, row_gesture, frame, time_ms, y_px, velocity_px_s, phase = fields
        if gesture is not None and row_gesture != gesture:
            continue
        rows.append(
            {
                "source": source,
                "gesture": row_gesture,
                "frame": int(frame),
                "time_ms": float(time_ms),
                "y_px": float(y_px),
                "velocity_px_s": float(velocity_px_s),
                "phase": phase,
            }
        )
    if not rows:
        raise SystemExit(f"No trace rows found in {path}")
    rows.sort(key=lambda row: row["time_ms"])
    return rows


def interpolate(rows, time_ms):
    if time_ms <= rows[0]["time_ms"]:
        return rows[0]["y_px"]
    if time_ms >= rows[-1]["time_ms"]:
        return rows[-1]["y_px"]
    for left, right in zip(rows, rows[1:]):
        if left["time_ms"] <= time_ms <= right["time_ms"]:
            span = right["time_ms"] - left["time_ms"]
            if span == 0:
                return right["y_px"]
            t = (time_ms - left["time_ms"]) / span
            return left["y_px"] + (right["y_px"] - left["y_px"]) * t
    return rows[-1]["y_px"]


def compare(reference, candidate):
    start = max(reference[0]["time_ms"], candidate[0]["time_ms"])
    end = min(reference[-1]["time_ms"], candidate[-1]["time_ms"])
    if end <= start:
        raise SystemExit("Traces do not overlap in time")

    samples = [row for row in candidate if start <= row["time_ms"] <= end]
    if not samples:
        raise SystemExit("No candidate samples overlap the reference trace")

    squared_error = 0.0
    max_error = 0.0
    for sample in samples:
        ref_y = interpolate(reference, sample["time_ms"])
        error = sample["y_px"] - ref_y
        squared_error += error * error
        max_error = max(max_error, abs(error))

    rms_error = math.sqrt(squared_error / len(samples))
    total_delta = (
        (candidate[-1]["y_px"] - candidate[0]["y_px"])
        - (reference[-1]["y_px"] - reference[0]["y_px"])
    )
    duration_delta = (
        candidate[-1]["time_ms"]
        - candidate[0]["time_ms"]
        - (reference[-1]["time_ms"] - reference[0]["time_ms"])
    )

    return {
        "samples": len(samples),
        "rms_offset_error_px": rms_error,
        "max_offset_error_px": max_error,
        "total_travel_delta_px": total_delta,
        "duration_delta_ms": duration_delta,
    }


def write_svg(path, reference, candidate):
    all_rows = reference + candidate
    max_time = max(row["time_ms"] for row in all_rows)
    max_y = max(row["y_px"] for row in all_rows)
    width = 960
    height = 360
    pad = 32

    def point(row):
        x = pad + (row["time_ms"] / max_time) * (width - pad * 2)
        y = height - pad - (row["y_px"] / max_y) * (height - pad * 2)
        return f"{x:.1f},{y:.1f}"

    ref_points = " ".join(point(row) for row in reference)
    cand_points = " ".join(point(row) for row in candidate)
    Path(path).write_text(
        f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}">
<rect width="100%" height="100%" fill="white"/>
<polyline points="{ref_points}" fill="none" stroke="#2563eb" stroke-width="2"/>
<polyline points="{cand_points}" fill="none" stroke="#dc2626" stroke-width="2"/>
<text x="{pad}" y="24" font-family="sans-serif" font-size="14" fill="#111827">blue=reference red=candidate</text>
</svg>
""",
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description="Compare inertia scroll trace CSV/logcat output.")
    parser.add_argument("reference", help="Reference trace CSV or logcat output")
    parser.add_argument("candidate", help="Candidate trace CSV or logcat output")
    parser.add_argument("--gesture", default="medium-flick")
    parser.add_argument("--svg", help="Optional SVG plot output path")
    args = parser.parse_args()

    reference = read_trace(args.reference, args.gesture)
    candidate = read_trace(args.candidate, args.gesture)
    result = compare(reference, candidate)

    print(f"gesture={args.gesture}")
    for key, value in result.items():
        if isinstance(value, float):
            print(f"{key}={value:.3f}")
        else:
            print(f"{key}={value}")

    if args.svg:
        write_svg(args.svg, reference, candidate)
        print(f"svg={args.svg}")


if __name__ == "__main__":
    main()
