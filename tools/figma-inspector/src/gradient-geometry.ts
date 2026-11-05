// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SnapshotTransform } from "./plugin/snapshot";

export function radialGeometry(transform: SnapshotTransform) {
    const [[a, c, e], [b, d, f]] = transform;
    const determinant = a * d - b * c;
    const xx = a * a + b * b;
    const yy = c * c + d * d;
    return {
        centerX: (d * (0.5 - e) - c * (0.5 - f)) / determinant,
        centerY: (a * (0.5 - f) - b * (0.5 - e)) / determinant,
        radiusX: 0.5 / Math.sqrt(xx),
        radiusY: 0.5 / Math.sqrt(yy),
        axisAligned: Math.abs(a * c + b * d) <= 1e-6 * Math.sqrt(xx * yy),
    };
}

export function conicGeometry(
    transform: SnapshotTransform,
    width: number,
    height: number,
) {
    const [[a, c], [b, d]] = transform;
    const xx = (a * a + b * b) / (width * width);
    const yy = (c * c + d * d) / (height * height);
    return {
        ...radialGeometry(transform),
        circular:
            a * d - b * c > 0 &&
            Math.abs(xx - yy) <= 1e-6 * Math.max(xx, yy) &&
            Math.abs((a * c + b * d) / (width * height)) <=
                1e-6 * Math.sqrt(xx * yy),
    };
}
