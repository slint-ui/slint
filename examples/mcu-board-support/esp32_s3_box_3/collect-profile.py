#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cspell:ignore IHDR IIBBBBB IDAT IEND

"""Collect BOX printer timing, deadline counters, and native RGB565 captures."""

import argparse
import io
import json
from pathlib import Path
import struct
import time
import zlib

import serial


def png(path, rows):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    pixels = bytearray()
    for y in range(240):
        pixels.append(0)
        words = rows[y]
        if len(words) != 1280:
            raise ValueError("incomplete capture row")
        for x in range(0, len(words), 4):
            word = int(words[x:x + 4], 16)
            pixels.extend(((word >> 11) * 255 // 31, (word >> 5 & 63) * 255 // 63, (word & 31) * 255 // 31))
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", 320, 240, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(pixels))
        + chunk(b"IEND", b"")
    )


def fields(line):
    return {k: int(v) for k, v in (part.split("=", 1) for part in line.split(",")[1:])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--port")
    source.add_argument("--log", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--captures", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=65)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    results, parity, sensors, rows = [], [], [], {}
    packets, ink_times = {}, []
    trace_truncated = False
    captures = 0
    deadline = time.monotonic() + args.timeout
    incoming = io.BytesIO(args.log.read_bytes()) if args.log else serial.Serial(args.port, 115200, timeout=.2, exclusive=True)
    with incoming as port, (args.output / "serial.log").open("w") as log:
        while time.monotonic() < deadline:
            raw = port.readline()
            if not raw and args.log:
                raise EOFError("incomplete recorded profile")
            line = raw.decode(errors="replace").strip()
            if not line:
                continue
            log.write(line + "\n")
            log.flush()
            if "Panic:" in line or "panicked" in line or "Guru Meditation" in line:
                raise RuntimeError(line)
            if line.startswith("PRINTER_RESULT,"):
                result = fields(line)
                result["fps"] = result["frames"] * 1_000_000 / result["elapsed_us"]
                result["draw_mean_us"] = result["draw_total_us"] / max(1, result["frames"])
                results.append(result)
                print(json.dumps(result), flush=True)
            elif line.startswith("PRINTER_CAPTURE_PARITY,"):
                parity.append(fields(line))
            elif line.startswith("BOX_IMU,"):
                sensors.append(line)
            elif line.startswith("BOX_INK_TIMES,"):
                ink_times.append(fields(line))
            elif line.startswith("PRINTER_PIPELINE_BEGIN,"):
                trace_truncated |= fields(line.split(",columns=", 1)[0])["truncated"] != 0
            elif line.startswith("PRINTER_PIPELINE,"):
                packet = [int(v) for v in line.split(",", 1)[1].split(";")]
                packets[tuple(packet[:4])] = packet
            elif line.startswith("PRINTER_CAPTURE_BEGIN,"):
                rows = {}
            elif line.startswith("PRINTER_CAPTURE,"):
                _, y, words = line.split(",", 2)
                rows[int(y)] = words
            elif line == "PRINTER_CAPTURE_END":
                png(args.output / f"capture-{captures}.png", rows)
                captures += 1
                if captures == args.captures:
                    break
        else:
            raise TimeoutError(f"received {captures}/{args.captures} captures")
    counters = ("repeated_panel_frames", "skipped_panel_cycles", "timeouts", "late_packets", "late_cycles", "truncated")
    passed = bool(results) and all(all(r[k] == 0 for k in counters) and 59.7 <= r["fps"] <= 60.2 for r in results)
    passed &= all(r["frames"] == r["presented"] == r["changed_hash"] for r in results)
    passed &= len(parity) == args.captures and all(p["differences"] == 0 for p in parity)
    passed &= not trace_truncated and bool(packets)
    passed &= all(fields(sensor)["errors"] == 0 for sensor in sensors)
    def delta(end, begin):
        value = (end - begin) & 0xffffffff
        return value if value < 0x80000000 else value - 0x100000000
    for result in results:
        group = [p for p in packets.values() if p[0] == result["mode"]]
        if not group:
            passed = False
            continue
        result["producer_mean_us"] = sum(delta(p[8], p[5]) for p in group) / len(group)
        result["ready_margin_min_us"] = min(delta(p[9], p[8]) for p in group)
        result["wire_mean_us"] = sum(delta(p[11], p[10]) for p in group) / len(group)
        passed &= all(p[2] == p[3] for p in group)
    summary = {"passed": passed, "results": results, "parity": parity, "sensors": sensors, "ink_times": ink_times}
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    if not passed:
        raise SystemExit("profile did not meet the panel cadence or pixel-parity requirements")


if __name__ == "__main__":
    main()
