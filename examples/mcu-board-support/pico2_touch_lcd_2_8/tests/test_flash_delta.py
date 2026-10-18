# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('delta', Path(__file__).parents[1] / 'flash-delta.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def block(address, byte, flags=0x2000, family=0xe48bff59):
    out = bytearray(512)
    struct.pack_into('<8I', out, 0, 0x0a324655, 0x9e5d5157, flags, address, 256, 0, 1, family)
    out[32:288] = bytes([byte]) * 256
    struct.pack_into('<I', out, 508, 0x0ab16f30)
    return bytes(out)


class DeltaTests(unittest.TestCase):
    def test_reconstructed_current_pages_and_neighbor_survive_sector_update(self):
        old = block(0x10000000, 1) + block(0x10100000, 2) + block(0x10100100, 3) + block(0x10200000, 4)
        new = block(0x10000000, 1) + block(0x10100000, 9) + block(0x10100100, 3) + block(0x10200000, 4)
        result, report = module.delta(old, new)
        merged = {h[3]: b[32:288] for h, b in module.blocks(old)}
        merged.update({h[3]: b[32:288] for h, b in module.blocks(result)})
        self.assertEqual(merged, {h[3]: b[32:288] for h, b in module.blocks(new)})
        headers = [h for h, _ in module.blocks(result)]
        self.assertEqual([h[3] for h in headers], [0x10000000, 0x10100000, 0x10100100])
        self.assertEqual([h[5:7] for h in headers], [(0, 3), (1, 3), (2, 3)])
        self.assertEqual(report['changed_sectors'], 2)

    def test_absolute_erratum_block_is_preserved(self):
        absolute = block(0x10ffff00, 0, 0xa000, 0xe48bff57)
        original = absolute + block(0x10000000, 1)
        result, _ = module.delta(original, original)
        self.assertEqual(result[:512], absolute)

    def test_invalid_image_cannot_produce_update(self):
        good = block(0x10000000, 1)
        for bad in [b'', good[:-1], block(0x20000000, 1), block(0x10000000, 1, 0x2001)]:
            with self.assertRaises(ValueError):
                module.delta(good, bad)


if __name__ == '__main__':
    unittest.main()
