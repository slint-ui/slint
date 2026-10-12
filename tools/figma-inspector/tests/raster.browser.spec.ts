// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { captureSource } from "../src/plugin/capture";
import { convertSnapshot } from "../src/preview/converter";
import {
    mountPreview,
    readFixture,
    canvasPixels,
    decodePng,
} from "./browser-harness";
import { compareScreenshotPixels } from "./screenshot-compare";

test("transformed vector capture renders PNG pixels instead of the SVG fast path", async () => {
    const p = await mountPreview();
    const stem = `fixtures/authored/raster/asymmetric-${p.win.devicePixelRatio}x`;
    const fixture = JSON.parse(await readFixture(`${stem}.json`));
    const base64 = await server.commands.readFile(`${stem}.png`, "base64");
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
        const diff = compareScreenshotPixels(
            await decodePng(base64),
            await canvasPixels(p),
            2,
        );
        expect(diff.differingPixels).toBe(0);
    }
});

test("radial panel gradients preserve elliptical placement and opaque child rows", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFixture("fixtures/source/radial-panel.json")),
    );
    if (!normalized.ok || normalized.empty)
        throw Error("Invalid radial fixture");
    const p = await mountPreview();
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
        const pixels = await canvasPixels(p);
        const scale = pixels.width / 32;
        const pixel = (x: number, y: number) => {
            const offset =
                (Math.floor(y * scale) * pixels.width + Math.floor(x * scale)) *
                4;
            return Array.from(pixels.data.slice(offset, offset + 4));
        };
        expect(pixel(16, 16)).toEqual([0, 0, 0, 255]);
        expect(pixel(16, 1)[0]).toBeGreaterThan(pixel(1, 16)[0] + 70);
        expect(pixel(1, 16)[2]).toBeGreaterThan(230);
    }
});

test("SVG paint bounds preserve overflow and layout placement", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFixture("fixtures/source/svg-paint-bounds.json")),
    );
    if (!normalized.ok || normalized.empty) throw Error("Invalid SVG fixture");
    const p = await mountPreview();
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
        const pixels = await canvasPixels(p);
        const scale = pixels.width / 80;
        const pixel = (x: number, y: number) => {
            const offset =
                (Math.floor(y * scale) * pixels.width + Math.floor(x * scale)) *
                4;
            return Array.from(pixels.data.slice(offset, offset + 4));
        };
        expect(pixel(16, 18)).toEqual([255, 0, 0, 255]);
        expect(pixel(24, 24)).toEqual([0, 0, 255, 255]);
        expect(pixel(48, 39)).toEqual([255, 0, 0, 255]);
        expect(pixel(14, 18)).toEqual([255, 255, 255, 255]);
    }
});

for (const name of ["asymmetric", "stroke", "mask-shadow"])
    test(`${name} matches independent authored Figma pixels`, async () => {
        const p = await mountPreview();
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
        const actual = await canvasPixels(p);
        const expected = await decodePng(
            await server.commands.readFile(`${stem}.png`, "base64"),
        );
        const diff = compareScreenshotPixels(expected, actual, 2);
        expect(diff.differingPixels).toBe(0);
        // The independent reference must contain visible content, not approve blank output.
        expect(expected.data.some((v, i) => i % 4 !== 3 && v < 200)).toBe(true);
    });
