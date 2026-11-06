// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import {
    type CanvasPixels,
    mountCanvasPreview,
    readFixture,
    decodePng,
    expectCanvasPixels,
} from "./browser-harness";

test("16px native paints, blur, shadows, and translucent dashes match Figma at 1×", async () => {
    const source = JSON.parse(
        await readFixture("fixtures/source/native-inner-shadow.json"),
    );
    const original = source.root;
    const white = { type: "SOLID", opacity: 1, color: { r: 1, g: 1, b: 1 } };
    const gradient = original.properties.fills[0];
    const colors = [
        { r: 1, g: 0, b: 0, a: 1 },
        { r: 0, g: 1, b: 0, a: 1 },
        { r: 0, g: 0, b: 1, a: 1 },
        { r: 1, g: 0, b: 0, a: 1 },
    ];
    const cells = [
        {
            x: 0,
            y: 0,
            cornerRadius: 2,
            fills: [gradient],
            effects: [
                {
                    ...original.properties.effects[0],
                    radius: 2,
                    spread: 1,
                    offset: { x: 1, y: 1 },
                },
            ],
        },
        {
            x: 8,
            y: 0,
            fills: [
                {
                    ...gradient,
                    type: "GRADIENT_ANGULAR",
                    gradientStops: colors.map((color, i) => ({
                        position: i / 3,
                        color,
                    })),
                },
            ],
        },
        {
            x: 0,
            y: 8,
            fills: [
                {
                    ...gradient,
                    type: "GRADIENT_RADIAL",
                    gradientTransform: [
                        [1, 0, 0],
                        [0, 2, -0.5],
                    ],
                    gradientStops: [
                        { position: 0, color: colors[0] },
                        { position: 1, color: colors[2] },
                    ],
                },
            ],
        },
        {
            x: 8,
            y: 8,
            fills: [{ ...white, opacity: 0.15 }],
            effects: [{ type: "BACKGROUND_BLUR", radius: 2, visible: true }],
            strokes: [
                { type: "SOLID", opacity: 0.5, color: { r: 1, g: 0, b: 0 } },
            ],
            strokeWeight: 1,
            strokeAlign: "INSIDE",
            dashPattern: [4, 4],
        },
    ];
    const rectangle = (properties: Record<string, unknown>, index: number) => ({
        ...original,
        id: `cell-${index}`,
        properties: {
            ...original.properties,
            width: 8,
            height: 8,
            cornerRadius: 0,
            effects: [],
            fills: [],
            ...properties,
        },
    });
    source.root = {
        ...original,
        type: "FRAME",
        properties: {
            ...original.properties,
            cornerRadius: 0,
            width: 16,
            height: 16,
            fills: [white],
            effects: [],
            clipsContent: true,
        },
        children: [
            rectangle(
                {
                    x: 0,
                    y: 0,
                    width: 12,
                    height: 16,
                    fills: [{ ...white, color: { r: 0, g: 0, b: 0 } }],
                },
                -1,
            ),
            ...cells.map(rectangle).reverse(),
        ],
    };
    // Exported from Figma, independently of the Slint renderer.
    const expected = await decodePng(
        await server.commands.readFile(
            "fixtures/authored/raster/native-inner-shadow.png",
            "base64",
        ),
    );
    const preview = await mountCanvasPreview();
    expect(preview.win.devicePixelRatio).toBe(1);
    let revision = 0;
    for (const target of ["preview", "export"] as const) {
        const normalized = await normalizeSource(source, target);
        if (!normalized.ok || normalized.empty)
            throw Error(JSON.stringify(normalized));
        const converted = convertSnapshot(normalized.snapshot);
        if (!converted.ok) throw Error(JSON.stringify(converted));
        expect(converted.source).toContain("inner-shadow-blur: 2px;");
        preview.send({
            type: "preview-source",
            revision: ++revision,
            source: converted.source,
            exportPackage: { source: converted.source, files: [] },
        });
        await preview.ready(revision);
        await expectCanvasPixels(preview, (actual) => {
            expect([actual.width, actual.height]).toEqual([16, 16]);
            for (const [cell, { x, y }] of cells.entries()) {
                let error = 0;
                for (let dy = 0; dy < 8; dy++)
                    for (let dx = 0; dx < 8; dx++) {
                        const offset = ((y + dy) * 16 + x + dx) * 4;
                        for (let channel = 0; channel < 4; channel++)
                            error += Math.abs(
                                actual.data[offset + channel] -
                                    expected.data[offset + channel],
                            );
                    }
                expect(error / 256, `Figma cell ${cell}`).toBeLessThan(3);
            }
            for (const [x, y] of [
                [8, 8],
                [15, 8],
                [8, 15],
                [15, 15],
            ]) {
                const offset = (y * 16 + x) * 4;
                for (let channel = 0; channel < 3; channel++)
                    expect(
                        Math.abs(
                            actual.data[offset + channel] -
                                expected.data[offset + channel],
                        ),
                    ).toBeLessThanOrEqual(6);
            }
        });
    }
});

test("translucent dashed borders composite once across alignments and caps", async () => {
    const source = JSON.parse(
        await readFixture("fixtures/source/native-inner-shadow.json"),
    );
    const child = structuredClone(source.root);
    child.id = "stroke";
    const white = { type: "SOLID", opacity: 1, color: { r: 1, g: 1, b: 1 } };
    Object.assign(source.root, { type: "FRAME", children: [child] });
    Object.assign(source.root.properties, {
        width: 16,
        height: 16,
        cornerRadius: 0,
        fills: [white],
        effects: [],
    });
    Object.assign(child.properties, {
        x: 4,
        y: 4,
        width: 8,
        height: 8,
        cornerRadius: 0,
        fills: [],
        effects: [],
        strokeWeight: 2,
        dashPattern: [6, 2],
        strokes: [{ type: "SOLID", opacity: 1, color: { r: 0, g: 0, b: 0 } }],
    });
    const p = await mountCanvasPreview();
    let revision = 0;
    for (const align of ["INSIDE", "CENTER", "OUTSIDE"])
        for (const cap of ["NONE", "ROUND", "SQUARE"]) {
            Object.assign(child.properties, {
                strokeAlign: align,
                strokeCap: cap,
            });
            const pixels: CanvasPixels[] = [];
            for (const opacity of [0, 1, 0.5]) {
                child.properties.strokes[0].opacity = opacity;
                const n = await normalizeSource(source);
                if (!n.ok || n.empty) throw Error(JSON.stringify(n));
                const c = convertSnapshot(n.snapshot);
                if (!c.ok) throw Error(JSON.stringify(c));
                p.send({
                    type: "preview-source",
                    revision: ++revision,
                    source: c.source,
                    exportPackage: { source: c.source, files: [] },
                });
                await p.ready(revision);
                pixels.push(
                    await expectCanvasPixels(p, (actual) =>
                        expect(actual.width).toBe(16),
                    ),
                );
            }
            let max = 0;
            for (let i = 0; i < pixels[0].data.length; i++)
                max = Math.max(
                    max,
                    Math.abs(
                        pixels[2].data[i] -
                            (pixels[0].data[i] + pixels[1].data[i]) / 2,
                    ),
                );
            expect(max, `${align} ${cap}`).toBeLessThanOrEqual(3);
        }
});
