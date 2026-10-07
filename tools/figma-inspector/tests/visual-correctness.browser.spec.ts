// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { packPreviewAssets } from "../src/asset-transport";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import { canvasPixels, mountPreview, readFixture } from "./browser-harness";

test("rotated bars occupy the bounds defined by Figma transforms", async () => {
    const normalized = await normalizeSource(
        JSON.parse(await readFixture("fixtures/source/rotated-bars.json")),
    );
    if (!normalized.ok || normalized.empty) throw Error("Invalid fixture");
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
        const pixel = (x: number, y: number) => {
            const offset = (y * pixels.width + x) * 4;
            return Array.from(pixels.data.slice(offset, offset + 4));
        };
        expect(pixel(15, 60)).toEqual([255, 0, 0, 255]);
        expect(pixel(15, 85)).toEqual([255, 255, 255, 255]);
        expect(pixel(75, 30)).toEqual([0, 255, 0, 255]);
        expect(pixel(75, 45)).toEqual([255, 255, 255, 255]);
        expect(pixel(35, 30)).toEqual([0, 0, 255, 255]);
        expect(pixel(45, 30)).toEqual([255, 255, 255, 255]);
    }
});

test("preview image URLs remain live until replacement or clearing", async () => {
    const p = await mountPreview();
    const browser = p.win as Window & typeof globalThis;
    const urls = browser.URL;
    const create = urls.createObjectURL.bind(urls);
    const revoke = urls.revokeObjectURL.bind(urls);
    const created: string[] = [];
    const revoked: string[] = [];
    urls.createObjectURL = (blob) => {
        const url = create(blob);
        if (blob instanceof browser.Blob && blob.type.startsWith("image/"))
            created.push(url);
        return url;
    };
    urls.revokeObjectURL = (url) => {
        if (created.includes(url)) revoked.push(url);
        revoke(url);
    };
    let previous: string[] = [];
    for (const [index, color] of ["red", "blue"].entries()) {
        const image = document.createElement("canvas");
        image.width = image.height = 8;
        const context = image.getContext("2d");
        if (!context) throw Error("Missing image context");
        context.fillStyle = color;
        context.fillRect(0, 0, 8, 8);
        const source = `export component Demo inherits Window { width: 16px; height: 16px; background: white; Image { x: 4px; y: 4px; width: 8px; height: 8px; source: @image-url("${image.toDataURL()}"); } }`;
        const packed = packPreviewAssets(source);
        expect(packed.assets).toHaveLength(1);
        const revision = index + 1;
        p.send({
            type: "preview-source",
            revision,
            source: packed,
            exportPackage: { source, files: [] },
        });
        await p.ready(revision);
        expect(created.length).toBeGreaterThan(previous.length);
        expect(revoked).toEqual(previous);
        const pixels = await canvasPixels(p);
        const offset = (8 * pixels.width + 8) * 4;
        expect(Array.from(pixels.data.slice(offset, offset + 4))).toEqual(
            index === 0 ? [255, 0, 0, 255] : [0, 0, 255, 255],
        );
        previous = [...created];
    }
    p.send({ type: "preview-clear", revision: 3 });
    await p.ready(3);
    await expect.poll(() => revoked).toEqual(created);
});
