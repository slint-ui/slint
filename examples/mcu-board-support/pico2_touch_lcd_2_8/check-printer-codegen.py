#!/usr/bin/env python3
"""Check printer release placement, arithmetic and immutable artwork identity."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile


def symbols(elf, llvm):
    rows = subprocess.check_output([str(llvm / 'llvm-nm'), '--print-size', '--demangle', str(elf)], text=True)
    return {parts[3]: (int(parts[0], 16), int(parts[1], 16))
            for line in rows.splitlines() if len(parts := line.split(maxsplit=3)) == 4
            and re.fullmatch('[0-9a-fA-F]+', parts[0])}


def artwork(elf, llvm, names):
    header = subprocess.check_output([str(llvm / 'llvm-objdump'), '-h', str(elf)], text=True)
    sections = []
    with tempfile.TemporaryDirectory() as temporary:
        for line in header.splitlines():
            parts = line.split()
            if len(parts) < 6 or parts[1] not in ('.assets', '.rodata') or int(parts[2], 16) == 0:
                continue
            raw = Path(temporary) / parts[1]
            subprocess.run([str(llvm / 'llvm-objcopy'), f'--dump-section={parts[1]}={raw}',
                            str(elf), str(Path(temporary) / 'unused.elf')], check=True)
            sections.append((int(parts[3], 16), raw.read_bytes()))
        result = {}
        for name, (address, size) in names.items():
            if not ('native_assets::data::' in name or re.search(r'SLINT_EMBEDDED_RESOURCE_\d+_DATA$', name)):
                continue
            for start, data in sections:
                offset = address - start
                if 0 <= offset and offset + size <= len(data):
                    result[name] = {'address': address, 'bytes': size,
                                    'sha256': hashlib.sha256(data[offset:offset + size]).hexdigest()}
                    break
            else:
                raise RuntimeError(f'Artwork is outside read-only sections: {name}')
        if len(result) < 14:
            raise RuntimeError('Expected native vial artwork and embedded UI resources')
        return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('elf', type=Path)
    p.add_argument('--llvm-bin', type=Path, required=True)
    p.add_argument('--compare-assets', type=Path)
    p.add_argument('--stable-asset-addresses', action='store_true')
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--picotool', default='picotool')
    a = p.parse_args()
    names = symbols(a.elf, a.llvm_bin)
    ram = {}
    for token in ['wire_format::pack', 'frame_queue::command', 'ScanClock>::sample', 'stock_clock::configure_flash']:
        matches = [(name, address) for name, (address, _) in names.items()
                   if token in name and 'Thunk' not in name]
        if len(matches) != 1 or not 0x20000000 <= matches[0][1] < 0x20080000:
            raise RuntimeError(f'Expected one SRAM function: {token}: {matches}')
        ram[matches[0][0]] = hex(matches[0][1])
    stack = names['_stack_start'][0] - names['_stack_end'][0]
    if stack < 24 * 1024:
        raise RuntimeError(f'Main stack reservation is too small: {stack}')
    with tempfile.TemporaryDirectory() as temporary:
        raw, decoded = [Path(temporary) / name for name in ('raw.dis', 'decoded.dis')]
        assembly = subprocess.check_output([str(a.llvm_bin / 'llvm-objdump'), '--disassemble', '--demangle',
                    '--triple=thumbv8m.main-none-eabihf', '--mcpu=cortex-m33', '--mattr=+dsp,+fp-armv8', str(a.elf)], text=True)
        raw.write_text(assembly)
        subprocess.run([a.picotool, 'coprodis', '--quiet', str(raw), str(decoded)], check=True)
        assembly = decoded.read_text()
    fpu = [line.strip() for line in assembly.splitlines() if re.search(r'\tv(?:mul|add|sub|div)\.f32\t', line)]
    if not fpu:
        raise RuntimeError('No hardware f32 arithmetic found')
    assets = artwork(a.elf, a.llvm_bin, names)
    if a.compare_assets:
        before = artwork(a.compare_assets, a.llvm_bin, symbols(a.compare_assets, a.llvm_bin))
        def fingerprints(table):
            return {name: (v['bytes'], v['sha256']) for name, v in table.items()}
        if fingerprints(before) != fingerprints(assets):
            raise RuntimeError('Native vial or embedded UI artwork changed')
        if a.stable_asset_addresses and {n: v['address'] for n, v in before.items()} != {n: v['address'] for n, v in assets.items()}:
            raise RuntimeError('Immutable artwork addresses moved')
    result = {'elf_sha256': hashlib.sha256(a.elf.read_bytes()).hexdigest(),
              'ram_functions': ram, 'main_stack_bytes': stack, 'hardware_f32': fpu[:12],
              'artwork': assets, 'compared_artwork': str(a.compare_assets) if a.compare_assets else None,
              'stable_asset_addresses': a.stable_asset_addresses}
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"Checked {len(assets)} unchanged artwork symbols, hardware f32, SRAM functions and {stack:,} bytes of main stack")


if __name__ == '__main__':
    main()
