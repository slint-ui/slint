#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Evaluate a distance-only profile against independent native probe speeds.

Pass a logcat capture containing ScrollCompare PHYSICS rows.
The five original probe speeds are fitting anchors; intermediate speeds validate
logarithmic interpolation. This does not validate duration or frame positions.
"""

import argparse
import json
import math
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    args = parser.parse_args()
    distances = {
        int(velocity): float(distance)
        for velocity, distance in re.findall(
            r"PHYSICS,(\d+),([\d.]+)", args.log.read_text()
        )
    }
    anchors = [376, 752, 1504, 3008, 6000]
    if any(velocity not in distances for velocity in anchors):
        parser.error("The capture must contain all five original probe speeds")
    results = []
    for velocity, distance in sorted(distances.items()):
        if velocity in anchors or not anchors[0] < velocity < anchors[-1]:
            continue
        lower = max(value for value in anchors if value < velocity)
        upper = min(value for value in anchors if value > velocity)
        t = math.log(velocity / lower) / math.log(upper / lower)
        predicted = math.exp(
            math.log(distances[lower]) * (1 - t) + math.log(distances[upper]) * t
        )
        results.append(dict(
            velocity=velocity,
            native_distance=distance,
            predicted_distance=predicted,
            error_percent=100 * (predicted / distance - 1),
        ))
    if not results:
        parser.error("No independent speeds between the fitting anchors were recorded")
    print(json.dumps(dict(
        anchors={value: distances[value] for value in anchors},
        held_out=results,
        max_absolute_error_percent=max(abs(row["error_percent"]) for row in results),
    ), indent=2))


if __name__ == "__main__":
    main()
