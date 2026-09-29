// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";
import { compareScreenshotPixels } from "./screenshot-compare";

const pixels = (...values: number[]) => new Uint8Array(values);

describe("screenshot pixel comparator", () => {
    test("accepts identical pixels only at the same dimensions", () => {
        const data = pixels(10, 20, 30, 255, 40, 50, 60, 255);
        expect(
            compareScreenshotPixels(
                { width: 2, height: 1, data },
                { width: 2, height: 1, data: pixels(...data) },
            ),
        ).toEqual({
            width: 2,
            height: 1,
            differingPixels: 0,
            maxChannelDelta: 0,
        });
    });

    test("rejects a one pixel shift or channel change", () => {
        const expected = pixels(0, 0, 0, 255, 255, 255, 255, 255);
        const shifted = pixels(255, 255, 255, 255, 0, 0, 0, 255);
        expect(
            compareScreenshotPixels(
                { width: 2, height: 1, data: expected },
                { width: 2, height: 1, data: shifted },
            ).differingPixels,
        ).toBe(2);
        expect(
            compareScreenshotPixels(
                { width: 2, height: 1, data: expected },
                {
                    width: 2,
                    height: 1,
                    data: pixels(0, 0, 1, 255, 255, 255, 255, 255),
                },
            ).maxChannelDelta,
        ).toBe(1);
    });

    test("rejects wrong dimensions instead of resizing", () => {
        expect(() =>
            compareScreenshotPixels(
                { width: 1, height: 1, data: pixels(0, 0, 0, 255) },
                {
                    width: 2,
                    height: 1,
                    data: pixels(0, 0, 0, 255, 0, 0, 0, 255),
                },
            ),
        ).toThrow("Screenshot dimensions differ");
    });

    test("keeps small channel tolerances distinct from larger pixel changes", () => {
        const expected = {
            width: 1,
            height: 1,
            data: pixels(100, 100, 100, 255),
        };
        const corner = {
            width: 1,
            height: 1,
            data: pixels(103, 100, 100, 255),
        };
        expect(
            compareScreenshotPixels(expected, corner, 2).differingPixels,
        ).toBe(1);
        expect(
            compareScreenshotPixels(expected, corner, 3).differingPixels,
        ).toBe(0);
        expect(
            compareScreenshotPixels(
                expected,
                {
                    ...corner,
                    data: pixels(104, 100, 100, 255),
                },
                3,
            ).differingPixels,
        ).toBe(1);
    });
});
