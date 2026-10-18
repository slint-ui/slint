#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Update changed flash sectors after checking the exact preceding firmware."""
import argparse
import sys
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import time

FLASH_START = 0x10000000
FLASH_END = 0x11000000


def blocks(raw):
    if len(raw) % 512:
        raise ValueError('Truncated UF2 file')
    result = []
    for offset in range(0, len(raw), 512):
        block = raw[offset:offset + 512]
        h = struct.unpack_from('<8I', block)
        if (h[:2] != (0x0a324655, 0x9e5d5157)
                or struct.unpack_from('<I', block, 508)[0] != 0x0ab16f30
                or h[4] != 256 or h[3] % 256
                or not FLASH_START <= h[3] < FLASH_END):
            raise ValueError('Expected absolute, flash-targeting 256-byte UF2 blocks')
        result.append((h, block))
    if not result:
        raise ValueError('Empty UF2 file')
    return result


def delta(previous, current):
    old_blocks, new_blocks = blocks(previous), blocks(current)
    families = {h[7] for h, _ in new_blocks if h[2] == 0x2000}
    if len(families) != 1:
        raise ValueError('Expected one application UF2 family')
    family = next(iter(families))
    if any(h[2] not in (0x2000, 0xa000) for h, _ in old_blocks + new_blocks):
        raise ValueError('Unsupported UF2 flags')
    old = {h[3]: b[32:288] for h, b in old_blocks if h[2] == 0x2000}
    changed = {h[3] & ~4095 for h, b in new_blocks
               if h[2] == 0x2000 and old.get(h[3]) != b[32:288]}
    # Keep the boot definition in the update and retain the tool-generated E10 block.
    changed.add(FLASH_START)
    selected = [(h, b) for h, b in new_blocks
                if h[2] == 0xa000 or h[3] & ~4095 in changed]
    count = sum(h[2] == 0x2000 for h, _ in selected)
    output = bytearray()
    index = 0
    for h, raw in selected:
        block = bytearray(raw)
        if h[2] == 0x2000:
            if h[7] != family:
                raise ValueError('Application family changed unexpectedly')
            struct.pack_into('<II', block, 20, index, count)
            index += 1
        output.extend(block)
    return bytes(output), dict(application_pages=count, changed_sectors=len(changed),
                               payload_bytes=len(selected) * 256,
                               full_payload_bytes=len(new_blocks) * 256)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--previous', required=True, type=Path)
    p.add_argument('--elf', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--picotool', default='picotool')
    p.add_argument('--objdump', default='llvm-objdump')
    p.add_argument('--objcopy', default='llvm-objcopy')
    p.add_argument('--serial')
    p.add_argument('--apply', action='store_true')
    a = p.parse_args()
    if a.apply and not a.serial:
        p.error('--apply requires --serial')
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        files = []
        for i, elf in enumerate([a.previous, a.elf]):
            target = Path(temporary) / f'{i}.uf2'
            subprocess.run([a.picotool, 'uf2', 'convert', str(elf), str(target),
                            '--family', 'rp2350-arm-s', '--abs-block'], check=True)
            files.append(target.read_bytes())
        raw, report = delta(*files)
        a.output.write_bytes(raw)
    report.update(previous_sha256=hashlib.sha256(a.previous.read_bytes()).hexdigest(),
                  elf_sha256=hashlib.sha256(a.elf.read_bytes()).hexdigest(), applied=False)
    report_path = a.output.with_suffix('.json')
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    if a.apply:
        # No writes occur until every section in the preceding ELF matches the device.
        spec = importlib.util.spec_from_file_location('flash_check', Path(__file__).with_name('check-printer-flash.py'))
        check = importlib.util.module_from_spec(spec)
        sys.path.insert(0, str(Path(__file__).parent))
        spec.loader.exec_module(check)
        import serial
        expected = check.sections(a.previous, a.objdump, a.objcopy)
        start = time.monotonic()
        with serial.Serial(check.find_port(a.serial), 115200, timeout=30) as port:
            report['flashinfo'] = check.command(port, 'flashinfo')
            for section in expected:
                answer = check.command(port, f"flashhash {section['offset']:x} {section['bytes']:x}")
                fields = dict(part.split('=', 1) for part in answer.split(',')[1:])
                if (not answer.startswith('PICO_FLASH_HASH,')
                        or int(fields['offset']) != section['offset']
                        or int(fields['bytes']) != section['bytes']
                        or int(fields['fnv32']) != section['expected_fnv32']):
                    raise RuntimeError('Device differs from the preceding ELF; use a complete flash')
        report['precheck_seconds'] = time.monotonic() - start
        start = time.monotonic()
        subprocess.run([a.picotool, 'load', str(a.output), '-t', 'uf2', '-u', '-v', '-x', '-f',
                        '--ser', a.serial], check=True)
        report.update(applied=True, load_verify_reboot_seconds=time.monotonic() - start)
        report_path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
