// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";

test("the radial affine map matches the native Figma gradient coordinates", async () => {
    const source = JSON.parse(
        await readFile("fixtures/source/radial-panel.json", "utf8"),
    );
    source.root.properties.width = 311;
    source.root.properties.height = 485;
    source.root.properties.fills[0].gradientTransform = [
        [-0.2872672379016876, 1.0995800495147705, 0.6101610660552979],
        [-1.394966959953308, 1.939241831022624e-16, 1.1885125637054443],
    ];
    const normalized = await normalizeSource(source);
    if (
        !normalized.ok ||
        normalized.empty ||
        !("fills" in normalized.snapshot.root)
    )
        throw Error("Invalid gradient");
    const fill = normalized.snapshot.root.fills[0];
    if (fill.kind !== "image") throw Error("Missing gradient fill");
    const svg = Buffer.from(fill.data, "base64").toString("utf8");
    const matrix = svg
        .match(/gradientTransform="matrix\(([^)]+)\)"/)?.[1]
        .split(" ")
        .map(Number);
    if (!matrix) throw Error("Missing affine map");
    const dimensions = [311, 485, 311, 485, 311, 485];
    const native = [0, 220.539, -111.472, -45.4158, 153.5, 13.9491];
    matrix.forEach((value, index) => {
        expect(value * dimensions[index]).toBeCloseTo(native[index], 2);
    });
});

test("radial panel fills survive conversion without baking in their children", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFile("fixtures/source/radial-panel.json", "utf8")),
    );
    if (!normalized.ok || normalized.empty)
        throw Error("Invalid panel fixture");
    expect(normalized.snapshot.root).toMatchObject({
        kind: "frame",
        fills: [
            { kind: "image", mimeType: "image/svg+xml", scaleMode: "STRETCH" },
        ],
        children: [{ id: "row", kind: "rectangle" }],
    });
    expect(
        normalized.warnings.some(
            (warning) => warning.code === "UNSUPPORTED_PAINT_IGNORED",
        ),
    ).toBe(false);
    expect(convertSnapshot(normalized.snapshot).ok).toBe(true);
});
