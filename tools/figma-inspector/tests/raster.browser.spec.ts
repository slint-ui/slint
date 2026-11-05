// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { captureSource } from "../src/plugin/capture";
import { convertSnapshot } from "../src/preview/converter";
import {
    mountCanvasPreview,
    readFixture,
    expectCanvasPixels,
    decodePng,
    readRasterReference,
} from "./browser-harness";
import { compareScreenshotPixels } from "./screenshot-compare";

test("native effects and paints match independently authored Figma regions", async () => {
    const p = await mountCanvasPreview();
    const density = p.win.devicePixelRatio;
    const source = JSON.parse(
        await readFixture("fixtures/source/native-paints-effects.json"),
    );
    const expected = await decodePng(
        await server.commands.readFile(
            `fixtures/authored/raster/native-paints-${density}x.png`,
            "base64",
        ),
    );
    let nextRevision = 0;
    for (const target of ["preview", "export"] as const) {
        const normalized = await normalizeSource(source, target);
        if (!normalized.ok || normalized.empty)
            throw Error(JSON.stringify(normalized));
        for (const specialize of [false, true]) {
            const converted = convertSnapshot(normalized.snapshot, {
                specialize,
            });
            if (!converted.ok) throw Error(JSON.stringify(converted));
            expect(converted.source).toContain("backdrop-blur: 4px;");
            expect(converted.source).toContain("inner-shadow-blur:");
            expect(converted.source).toContain("stroke-dash-array:");
            expect(converted.source).toContain("@conic-gradient(");
            const revision = ++nextRevision;
            p.send({
                type: "preview-source",
                revision,
                source: converted.source,
                exportPackage: { source: converted.source, files: [] },
            });
            await p.ready(revision);
            await expectCanvasPixels(p, (actual) => {
                expect([actual.width, actual.height]).toEqual([
                    expected.width,
                    expected.height,
                ]);
                const errors = Array.from({ length: 12 }, (_, cell) => {
                    let delta = 0;
                    const size = 64 * density;
                    for (let y = 0; y < size; y++)
                        for (let x = 0; x < size; x++) {
                            const offset =
                                ((Math.floor(cell / 4) * size + y) *
                                    actual.width +
                                    (cell % 4) * size +
                                    x) *
                                4;
                            for (let channel = 0; channel < 3; channel++)
                                delta += Math.abs(
                                    actual.data[offset + channel] -
                                        expected.data[offset + channel],
                                );
                        }
                    return delta / (size * size * 3);
                });
                for (const [cell, error] of errors.entries()) {
                    // Font substitution and outlined-SVG resampling differ from Figma's glyph raster.
                    // Check the text's outline and fill below.
                    if (cell === 10) continue;
                    expect(
                        error,
                        `${target}, Figma region ${cell}: ${errors.join(", ")}`,
                    ).toBeLessThan(3);
                }
                // Native export uses embedded Inter, which can differ from Figma's font version.
                // Require both the dark outline and yellow fill despite those glyph differences.
                const ink = (pixels: typeof actual) => {
                    let outline = 0;
                    let fill = 0;
                    for (let y = 128 * density; y < 192 * density; y++)
                        for (let x = 128 * density; x < 192 * density; x++) {
                            const offset = (y * pixels.width + x) * 4;
                            const [r, g, b] = pixels.data.slice(
                                offset,
                                offset + 3,
                            );
                            if (r < 65 && g < 100 && b > 45 && b < 135)
                                outline++;
                            if (r > 220 && g > 170 && b < 100) fill++;
                        }
                    return { outline, fill };
                };
                const actualInk = ink(actual);
                const expectedInk = ink(expected);
                expect(actualInk.outline).toBeGreaterThan(
                    expectedInk.outline * 0.65,
                );
                expect(actualInk.outline).toBeLessThan(
                    expectedInk.outline * 1.35,
                );
                expect(actualInk.fill).toBeGreaterThan(expectedInk.fill * 0.65);
            });
        }
    }
});

test("transformed vector capture renders PNG pixels instead of the SVG fast path", async () => {
    const p = await mountCanvasPreview();
    const stem = `fixtures/authored/raster/asymmetric-${p.win.devicePixelRatio}x`;
    const fixture = JSON.parse(await readFixture(`${stem}.json`));
    const base64 = await readRasterReference(
        "asymmetric",
        p.win.devicePixelRatio,
    );
    const bytes = Uint8Array.from(atob(base64), (value) => value.charCodeAt(0));
    const node = {
        ...fixture.root.properties,
        id: "transformed-vector",
        name: "Transformed vector",
        type: "VECTOR",
        absoluteTransform: [
            [1, 0.5, 0],
            [0, 1, 0],
        ],
        rotation: 0,
        effects: [],
        fills: [{ type: "SOLID", color: { r: 0, g: 0, b: 1 } }],
        strokes: [],
    } as unknown as SceneNode;
    const captured = await captureSource(
        node,
        Symbol("mixed"),
        async () =>
            '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="32"><rect width="24" height="32" fill="blue"/></svg>',
        undefined,
        async () => bytes,
        p.win.devicePixelRatio,
    );
    const normalized = await normalizeSource(captured.source);
    if (!normalized.ok || normalized.empty)
        throw Error("Invalid transformed capture");
    for (const specialize of [false, true]) {
        const result = convertSnapshot(normalized.snapshot, { specialize });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        const revision = specialize ? 2 : 1;
        p.send({
            type: "preview-source",
            revision,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(revision);
        const expected = await decodePng(base64);
        await expectCanvasPixels(p, (pixels) => {
            expect(
                compareScreenshotPixels(expected, pixels, 2).differingPixels,
            ).toBe(0);
        });
    }
});

test("radial panel gradients preserve elliptical placement and opaque child rows", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFixture("fixtures/source/radial-panel.json")),
    );
    if (!normalized.ok || normalized.empty)
        throw Error("Invalid radial fixture");
    const p = await mountCanvasPreview();
    for (const specialize of [false, true]) {
        const result = convertSnapshot(normalized.snapshot, { specialize });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        const revision = specialize ? 2 : 1;
        p.send({
            type: "preview-source",
            revision,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(revision);
        await expectCanvasPixels(p, (pixels) => {
            const scale = pixels.width / 32;
            const pixel = (x: number, y: number) => {
                const offset =
                    (Math.floor(y * scale) * pixels.width +
                        Math.floor(x * scale)) *
                    4;
                return Array.from(pixels.data.slice(offset, offset + 4));
            };
            expect(pixel(16, 16)).toEqual([0, 0, 0, 255]);
            expect(pixel(16, 1)[0]).toBeGreaterThan(pixel(1, 16)[0] + 70);
            expect(pixel(1, 16)[2]).toBeGreaterThan(230);
        });
    }
});

test("SVG paint bounds preserve overflow and layout placement", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFixture("fixtures/source/svg-paint-bounds.json")),
    );
    if (!normalized.ok || normalized.empty) throw Error("Invalid SVG fixture");
    const p = await mountCanvasPreview();
    for (const specialize of [false, true]) {
        const converted = convertSnapshot(normalized.snapshot, { specialize });
        if (!converted.ok) throw Error(JSON.stringify(converted.diagnostics));
        const revision = specialize ? 2 : 1;
        p.send({
            type: "preview-source",
            revision,
            source: converted.source,
            exportPackage: { source: converted.source, files: [] },
        });
        await p.ready(revision);
        await expectCanvasPixels(p, (pixels) => {
            const scale = pixels.width / 80;
            const pixel = (x: number, y: number) => {
                const offset =
                    (Math.floor(y * scale) * pixels.width +
                        Math.floor(x * scale)) *
                    4;
                return Array.from(pixels.data.slice(offset, offset + 4));
            };
            expect(pixel(16, 18)).toEqual([255, 0, 0, 255]);
            expect(pixel(24, 24)).toEqual([0, 0, 255, 255]);
            expect(pixel(48, 39)).toEqual([255, 0, 0, 255]);
            expect(pixel(14, 18)).toEqual([255, 255, 255, 255]);
        });
    }
});

for (const name of ["asymmetric", "stroke", "mask-shadow"])
    test(`${name} matches independent authored Figma pixels`, async () => {
        const p = await mountCanvasPreview();
        const density = p.win.devicePixelRatio;
        const stem = `fixtures/authored/raster/${name}-${density}x`;
        const source = JSON.parse(await readFixture(`${stem}.json`));
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Expected authored capture");
        const result = convertSnapshot(normalized.snapshot);
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: 1,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(1);
        const expected = await decodePng(
            await readRasterReference(name, density),
        );
        await expectCanvasPixels(p, (pixels) => {
            expect(
                compareScreenshotPixels(expected, pixels, 2).differingPixels,
            ).toBe(0);
        });
        // The independent reference must contain visible content, not approve blank output.
        expect(expected.data.some((v, i) => i % 4 !== 3 && v < 200)).toBe(true);
    });
