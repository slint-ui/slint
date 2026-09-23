# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cspell:ignore logcat

import re
import sys
from pathlib import Path


log = Path(sys.argv[1]).read_text()
samples = re.findall(
    r"ScrollCompare[^\n]*:\s*A,[-0-9.]+,\d+,([-0-9.]+),([-0-9.]+)", log
)
if not samples:
    sys.exit("No Android/Slint offset samples appeared in logcat")

android, slint = ([float(pair[index]) for pair in samples] for index in (0, 1))
for name, values in (("Android", android), ("Slint", slint)):
    change = max(values) - min(values)
    print(f"{name}: {len(values)} samples, offset change {change:.1f} dp")
    if change < 20:
        sys.exit(f"{name} did not scroll after the shared swipe")
