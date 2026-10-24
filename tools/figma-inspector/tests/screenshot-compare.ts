// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export type ScreenshotPixels = {
    readonly width: number;
    readonly height: number;
    readonly data: Uint8Array | Uint8ClampedArray;
};

export type ScreenshotComparison = {
    readonly width: number;
    readonly height: number;
    readonly differingPixels: number;
    readonly maxChannelDelta: number;
};

/** Compare decoded RGBA screenshots without resizing, filtering, or alignment search. */
export function compareScreenshotPixels(
    expected: ScreenshotPixels,
    actual: ScreenshotPixels,
    maxAllowedChannelDelta = 0,
): ScreenshotComparison {
    if (expected.width !== actual.width || expected.height !== actual.height) {
        throw new Error(
            `Screenshot dimensions differ: ${expected.width}x${expected.height} versus ${actual.width}x${actual.height}`,
        );
    }
    if (expected.data.length !== actual.data.length)
        throw new Error("Screenshot pixel buffers have different lengths");

    let differingPixels = 0;
    let maxChannelDelta = 0;
    for (let index = 0; index < expected.data.length; index += 4) {
        const delta = Math.max(
            Math.abs(expected.data[index] - actual.data[index]),
            Math.abs(expected.data[index + 1] - actual.data[index + 1]),
            Math.abs(expected.data[index + 2] - actual.data[index + 2]),
            Math.abs(expected.data[index + 3] - actual.data[index + 3]),
        );
        if (delta > maxAllowedChannelDelta) differingPixels += 1;
        maxChannelDelta = Math.max(maxChannelDelta, delta);
    }
    return {
        width: expected.width,
        height: expected.height,
        differingPixels,
        maxChannelDelta,
    };
}
