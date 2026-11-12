# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Adds `idf.py trace`, which receives the capture that CONFIG_SLINT_TRACING records and opens it in Tracy."""

import json
import os
import queue
import shutil
import struct
import subprocess
import sys
import threading
import time
from typing import Any

# The packet and capture format is described in api/cpp/trace.rs.
PACKET_PREFIX = b"\x1eB"
END_PREFIX = b"\x1eE"
ESCAPE = 0x1B
CHUNK_BYTES = 480

ENTER, EXIT, EVENT, EXTENDED = 0, 1, 2, 3
NAME, FIELD, CLOCK, SPAN_COST, TRACK_SPAN, CORE = 0, 1, 2, 3, 4, 5
# Tracy thread names, by track number. TRACK_SPAN and CORE records name the track they're on.
TRACK_NAMES = {0: "slint", 1: "panel transfer", 2: "render (core 1)"}


def unescape(data: bytes) -> bytes:
    out = bytearray()
    escaped = False
    for byte in data:
        if escaped:
            out.append(byte ^ 0x40)
            escaped = False
        elif byte == ESCAPE:
            escaped = True
        else:
            out.append(byte)
    return bytes(out)


def varint(data: bytes, pos: int) -> tuple[int, int]:
    value = shift = 0
    while True:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if byte < 0x80:
            return value, pos


def collect_names(words: tuple[int, ...]) -> dict[int, str]:
    """Returns the names of the whole capture, since a core can name what another one records."""
    names: dict[int, str] = {}
    pos = 0
    while pos < len(words):
        header = words[pos]
        if header & 3 != EXTENDED:
            pos += 2
            continue
        payload = words[pos + 1 : pos + 1 + (header >> 8)]
        pos += 1 + (header >> 8)
        if (header >> 2) & 0x3F == NAME and len(payload) >= 2:
            address, length = payload[0], payload[1]
            text = struct.pack(f"<{len(payload) - 2}I", *payload[2:])[:length]
            names[address] = text.decode(errors="replace")
    return names


def decode(capture: bytes) -> list[dict[str, Any]]:
    """Turns the capture into Chrome trace events, with times in microseconds."""
    events: list[dict[str, Any]] = []
    words = struct.unpack(f"<{len(capture) // 4}I", capture[: len(capture) // 4 * 4])
    names = collect_names(words)
    # The track of the core whose records are being decoded.
    tid = 0
    ticks_per_us = 1
    span_cost: int | None = None
    fields: dict[str, int] = {}
    # Id, begin event, and the value of `spans_entered` at entry, for each open span.
    open_spans: list[tuple[int, dict[str, Any], int]] = []
    spans_entered = 0
    wraps = 0
    last_cycles: int | None = None
    time_us = 0.0

    def end_span(begin: dict[str, Any], entered: int) -> None:
        # Spans recorded inside this one, and their estimated recording cost, which is part of
        # this span's duration.
        nested = spans_entered - entered
        begin["args"]["nested_spans"] = nested
        if span_cost is not None:
            begin["args"]["trace_cost_us"] = round((nested + 1) * span_cost / ticks_per_us, 3)
        events.append({"ph": "E", "ts": time_us, "pid": 1, "tid": tid})

    pos = 0
    while pos < len(words):
        header = words[pos]
        kind = header & 3
        # An incomplete capture can end in the middle of a record.
        if pos + 1 + (header >> 8 if kind == EXTENDED else 1) > len(words):
            break
        if kind == EXTENDED:
            record_type = (header >> 2) & 0x3F
            payload = words[pos + 1 : pos + 1 + (header >> 8)]
            pos += 1 + (header >> 8)
            if record_type == NAME:
                pass
            elif record_type == CORE:
                # Spans of the previous core still open when its records ended.
                for _, begin, entered in reversed(open_spans):
                    end_span(begin, entered)
                open_spans = []
                fields = {}
                wraps = 0
                last_cycles = None
                tid = payload[0]
            elif record_type == FIELD:
                value = payload[1] | payload[2] << 32
                if value >= 1 << 63:
                    value -= 1 << 64
                fields[names.get(payload[0], f"0x{payload[0]:x}")] = value
            elif record_type == CLOCK:
                ticks_per_us = payload[0]
            elif record_type == SPAN_COST:
                span_cost = payload[0]
            elif record_type == TRACK_SPAN:
                identifier, begin_cycles, end_cycles, track = payload[:4]
                name = names.get(identifier, f"0x{identifier:x}")

                # Both ends lie close to the last recorded time, before or after it.
                def to_us(cycles: int) -> float:
                    if last_cycles is None:
                        return cycles / ticks_per_us
                    delta = (cycles - last_cycles) & 0xFFFFFFFF
                    if delta >= 1 << 31:
                        delta -= 1 << 32
                    return time_us + delta / ticks_per_us

                begin = {"name": name, "ph": "B", "ts": to_us(begin_cycles), "pid": 1}
                events.append({**begin, "tid": track, "args": fields})
                fields = {}
                events.append({"ph": "E", "ts": to_us(end_cycles), "pid": 1, "tid": track})
            continue

        identifier, cycles = header & ~3, words[pos + 1]
        pos += 2
        # The device sends the low 32 bits of the cycle counter, which wraps every few seconds.
        if last_cycles is not None and cycles < last_cycles:
            wraps += 1
        last_cycles = cycles
        time_us = ((wraps << 32) + cycles) / ticks_per_us
        name = names.get(identifier, f"0x{identifier:x}")
        if kind == ENTER:
            begin = {"name": name, "ph": "B", "ts": time_us, "pid": 1, "tid": tid, "args": fields}
            fields = {}
            events.append(begin)
            spans_entered += 1
            open_spans.append((identifier, begin, spans_entered))
        elif kind == EXIT:
            # A span still open inside this one was entered before the capture started.
            if any(open_id == identifier for open_id, _, _ in open_spans):
                while True:
                    open_id, begin, entered = open_spans.pop()
                    end_span(begin, entered)
                    if open_id == identifier:
                        break
        else:
            event = {"name": name, "ph": "i", "s": "t", "ts": time_us, "pid": 1, "tid": tid}
            if fields:
                event["args"] = fields
                fields = {}
            events.append(event)

    # Spans still open when the capture ended.
    for _, begin, entered in reversed(open_spans):
        end_span(begin, entered)
    return events


def chrome_trace(events: list[dict[str, Any]]) -> dict[str, Any]:
    tracks = sorted({event["tid"] for event in events} | {0})
    metadata = [
        {"ph": "M", "name": "thread_name", "pid": 1, "tid": track,
         "args": {"name": TRACK_NAMES.get(track, f"track {track}")}}
        for track in tracks
    ]
    # TRACK_SPAN records arrive after the time they cover. Sorting is stable, so events with equal
    # timestamps stay in recording order, which Tracy's importer keeps.
    return {"traceEvents": metadata + sorted(events, key=lambda event: event["ts"])}


def open_in_tracy(json_path: str, tracy_path: str) -> None:
    importer = shutil.which("tracy-import-chrome")
    if importer is None:
        print(f"tracy-import-chrome not found; open {json_path} in Perfetto or import it into Tracy yourself")
        return
    subprocess.run([importer, json_path, tracy_path], check=True, stdout=subprocess.DEVNULL)
    profiler = shutil.which("tracy-profiler") or shutil.which("tracy")
    if profiler is None:
        print(f"Tracy not found; open {tracy_path} in Tracy")
        return
    subprocess.Popen(
        [profiler, tracy_path], start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    print(f"Opened {tracy_path} in Tracy")


def receive(port: str, reset: bool) -> tuple[bytes, str | None]:
    """Prints the console output, and returns the capture up to the first missing or damaged part
    together with a description of that problem, if any."""
    import serial

    ser = serial.Serial()
    ser.port = port
    ser.timeout = 0.1
    ser.dtr = False
    ser.rts = False
    ser.open()
    if reset:
        ser.rts = True
        time.sleep(0.2)
        ser.rts = False

    # Reading runs in its own thread so that handling packets never delays it.
    chunks: queue.Queue[bytes] = queue.Queue()
    stop_reading = threading.Event()

    def read_port() -> None:
        while not stop_reading.is_set():
            data = ser.read(ser.in_waiting or 1)
            if data:
                chunks.put(data)

    reader = threading.Thread(target=read_port, daemon=True)
    reader.start()

    capture = bytearray()
    problem: str | None = None
    next_sequence = 0
    status_shown = False
    last_status = 0.0
    pending = b""
    try:
        while True:
            try:
                pending += chunks.get(timeout=0.1)
            except queue.Empty:
                pass
            *lines, pending = pending.split(b"\n")
            for line in lines:
                line = line.rstrip(b"\r")
                text, _, packet = line.partition(PACKET_PREFIX)
                if END_PREFIX in text:
                    return bytes(capture), problem
                if packet and problem is None:
                    data = unescape(packet)
                    try:
                        sequence, start = varint(data, 0)
                    except IndexError:
                        sequence, start = -1, 0
                    if sequence != next_sequence:
                        problem = f"packet {next_sequence} is missing or damaged"
                    # Every packet but the last carries a full chunk.
                    elif len(capture) % CHUNK_BYTES or len(data) - start > CHUNK_BYTES:
                        problem = f"packet {next_sequence - 1} has the wrong length"
                    else:
                        next_sequence += 1
                        capture += data[start:]
                    if problem is not None:
                        # Keep only the whole chunks before the problem.
                        del capture[len(capture) - len(capture) % CHUNK_BYTES :]
                if text or not packet:
                    if status_shown:
                        sys.stderr.write("\r\033[K")
                        status_shown = False
                    sys.stdout.buffer.write(text + b"\n")
                    sys.stdout.flush()
            if capture and time.monotonic() - last_status > 0.5:
                sys.stderr.write(f"\r\033[K[slint trace] received {len(capture) // 1024} KiB")
                sys.stderr.flush()
                status_shown = True
                last_status = time.monotonic()
    except KeyboardInterrupt:
        return bytes(capture), problem or "the capture was interrupted"
    finally:
        stop_reading.set()
        reader.join()
        ser.close()
        sys.stderr.write("\n")


def action_extensions(base_actions: dict, project_path: str) -> dict:
    def trace(action: str, ctx: Any, args: Any, no_reset: bool) -> None:
        if args.port is None:
            from idf_py_actions.tools import get_default_serial_port

            args.port = get_default_serial_port()

        capture, problem = receive(args.port, reset=not no_reset)
        with open(os.path.join(args.build_dir, "slint-trace.bin"), "wb") as f:
            f.write(capture)
        if problem is not None:
            print(
                f"The capture is incomplete: {problem}. "
                f"Using the {len(capture) // 1024} KiB that arrived before it."
            )
        events = decode(capture)
        if not events:
            print("The capture has no spans. Enable CONFIG_SLINT_TRACING, then build and flash again.")
            return

        json_path = os.path.join(args.build_dir, "slint-trace.json")
        with open(json_path, "w") as f:
            json.dump(chrome_trace(events), f)
        print(f"Wrote {len(events)} events to {json_path}")
        open_in_tracy(json_path, os.path.join(args.build_dir, "slint-trace.tracy"))

    return {
        "version": "1",
        "actions": {
            "trace": {
                "callback": trace,
                "help": "Receive the capture that CONFIG_SLINT_TRACING records, then open it in Tracy.",
                "options": [
                    {
                        "names": ["--no-reset"],
                        "is_flag": True,
                        "help": "Attach to the running app instead of resetting it first.",
                    },
                ],
                "order_dependencies": ["flash", "app-flash"],
            }
        },
    }
