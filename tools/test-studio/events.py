# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import time


def read(path, *, offset=0, sequence=0, final=False):
    events = []
    warnings = []
    with path.open() as stream:
        stream.seek(offset)
        while True:
            start = stream.tell()
            line = stream.readline()
            if not line:
                break
            if not line.endswith("\n") and not final:
                stream.seek(start)
                break
            offset = stream.tell()
            try:
                event = json.loads(line)
                current = event.get("sequence")
                if (
                    event.get("version") != 1
                    or event.get("run_id") != path.parent.name
                    or not isinstance(current, int)
                    or current <= sequence
                ):
                    raise ValueError("Unsupported event envelope")
                if current != sequence + 1:
                    warnings.append(
                        f"Event sequence skipped from {sequence} to {current}"
                    )
                sequence = current
                events.append(event)
            except (
                json.JSONDecodeError,
                ValueError,
                TypeError,
                AttributeError,
            ) as error:
                warnings.append(f"Damaged history event: {error}")
        offset = stream.tell()
    return events, offset, sequence, warnings


class Writer:
    def __init__(self, path):
        self.path = path
        self.sequence = 0
        if path.exists():
            for line in path.read_text().splitlines():
                try:
                    self.sequence = max(
                        self.sequence, json.loads(line).get("sequence", 0)
                    )
                except ValueError:
                    pass

    def emit(self, kind, **data):
        self.sequence += 1
        event = {
            "version": 1,
            "run_id": self.path.parent.name,
            "sequence": self.sequence,
            "timestamp": time.time(),
            "kind": kind,
            **data,
        }
        needs_newline = False
        if self.path.exists() and self.path.stat().st_size:
            with self.path.open("rb") as tail:
                tail.seek(-1, 2)
                needs_newline = tail.read(1) != b"\n"
        with self.path.open("a") as stream:
            if needs_newline:
                stream.write("\n")
            stream.write(json.dumps(event) + "\n")
            stream.flush()
        return event
