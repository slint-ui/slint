#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# cSpell:ignore objcopy flashinfo flashhash

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

import serial
from printer_protocol import command, find_port


def sections(elf, objdump, objcopy):
    headers = subprocess.check_output([objdump, "-h", str(elf)], text=True)
    result = []
    with tempfile.TemporaryDirectory() as temporary:
        for line in headers.splitlines():
            fields = line.split()
            if len(fields) < 6 or fields[1] not in (
                ".vector_table", ".start_block", ".text", ".rodata", ".data"
            ):
                continue
            name = fields[1]
            length = int(fields[2], 16)
            offset = int(fields[4], 16) - 0x10000000
            if length == 0 or length % 4 or offset < 0 or offset + length > 16 * 1024 * 1024:
                raise RuntimeError(f"Unsupported flash section: {line}")
            raw = Path(temporary) / "section.bin"
            subprocess.run(
                [objcopy, f"--dump-section={name}={raw}", str(elf),
                 str(Path(temporary) / "unused.elf")], check=True,
            )
            data = raw.read_bytes()
            if len(data) != length:
                raise RuntimeError(f"Unexpected section length: {name}")
            value = 0x811c9dc5
            for byte in data:
                value = ((value ^ byte) * 0x01000193) & 0xffffffff
            result.append(dict(section=name, offset=offset, bytes=length, expected_fnv32=value))
    if len(result) != 5:
        raise RuntimeError("ELF does not contain the five expected flash sections")
    return result


def main():
    parser = argparse.ArgumentParser(description="Compare uncached Pico flash reads with its ELF")
    parser.add_argument("--serial", required=True)
    parser.add_argument("--elf", type=Path, required=True)
    parser.add_argument("--objdump", default="llvm-objdump")
    parser.add_argument("--objcopy", default="llvm-objcopy")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    expected = sections(args.elf, args.objdump, args.objcopy)
    result = dict(elf_sha256=hashlib.sha256(args.elf.read_bytes()).hexdigest(), checks=[])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with serial.Serial(find_port(args.serial), 115200, timeout=30, write_timeout=2) as port:
        result["flashinfo"] = command(port, "flashinfo")
        for repeat in range(args.repeats):
            for section in expected:
                response = command(port, f"flashhash {section['offset']:x} {section['bytes']:x}")
                if not response.startswith("PICO_FLASH_HASH,"):
                    raise RuntimeError(response)
                fields = dict(part.split("=", 1) for part in response.split(",")[1:])
                value = int(fields["fnv32"])
                result["checks"].append(dict(repeat=repeat, **section, actual_fnv32=value))
                args.output.write_text(json.dumps(result, indent=2) + "\n")
                if (int(fields["offset"]) != section["offset"]
                        or int(fields["bytes"]) != section["bytes"]
                        or value != section["expected_fnv32"]):
                    raise RuntimeError(f"Flash differs from the ELF: {section['section']}")
            print(f"Pass {repeat + 1}: section hashes match over {sum(s['bytes'] for s in expected):,} bytes", flush=True)


if __name__ == "__main__":
    main()
