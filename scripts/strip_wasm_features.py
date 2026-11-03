# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Removes feature names that Emscripten 3.1.56 doesn't know from the target_features
section of WebAssembly object files, keeping every other feature.

Usage: strip_wasm_features.py <object files...>
"""

import sys

UNKNOWN = {b"bulk-memory-opt", b"call-indirect-overlong"}


def read_leb(data, pos):
    result = shift = 0
    while True:
        byte = data[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return result, pos


def write_leb(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def rewrite(data):
    if data[:4] != b"\0asm":
        return data, 0
    out = bytearray(data[:8])
    pos = 8
    removed = 0
    while pos < len(data):
        section_id = data[pos]
        size, body = read_leb(data, pos + 1)
        end = body + size
        if section_id == 0:
            name_len, name_pos = read_leb(data, body)
            name = data[name_pos : name_pos + name_len]
            if name == b"target_features":
                p = name_pos + name_len
                count, p = read_leb(data, p)
                kept = []
                for _ in range(count):
                    prefix = data[p]
                    feature_len, fp = read_leb(data, p + 1)
                    feature = data[fp : fp + feature_len]
                    p = fp + feature_len
                    if feature in UNKNOWN:
                        removed += 1
                    else:
                        kept.append(bytes([prefix]) + write_leb(feature_len) + feature)
                payload = (
                    write_leb(len(name)) + name + write_leb(len(kept)) + b"".join(kept)
                )
                out += bytes([0]) + write_leb(len(payload)) + payload
                pos = end
                continue
        out += data[pos:end]
        pos = end
    return bytes(out), removed


total = 0
for path in sys.argv[1:]:
    with open(path, "rb") as f:
        data = f.read()
    new, removed = rewrite(data)
    if removed:
        with open(path, "wb") as f:
            f.write(new)
        total += removed
print(f"removed {total} feature entries from {len(sys.argv) - 1} objects")
