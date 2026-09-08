// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { normalizeSource } from "../src/plugin/normalize";
import type { SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import { validateSnapshot } from "../src/plugin/snapshot";

describe("image-crop-normalization", () => {
    test("authored crop removes the bitmap border instead of inverting the transform", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/image-crop.json", "utf8"),
        );
        const original = structuredClone(source);
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw new Error("Expected QR snapshot");
        expect(source).toEqual(original);
        expect(result.snapshot.root).toMatchObject({
            width: 24,
            height: 24,
            fills: [
                {
                    kind: "image",
                    crop: [0.25, 0.25, 0.5, 0.5],
                },
            ],
        });
        const converted = convertSnapshot(result.snapshot);
        if (!converted.ok) throw new Error("Expected Slint source");
        expect(converted.source).toContain("source-clip-width: 12;");
        expect(converted.source).not.toContain("source-clip-width: 24;");
    });
});

describe("image-grayscale", () => {
    test.each([
        { filters: { saturation: -1 }, grayscale: true, ignored: false },
        { filters: { saturation: 0 }, grayscale: false, ignored: false },
        { filters: { saturation: -0.5 }, grayscale: false, ignored: true },
        {
            filters: { saturation: -1, contrast: 0.5 },
            grayscale: true,
            ignored: true,
        },
    ])(
        "full desaturation is supported without hiding other unsupported filters: $filters",
        async ({ filters, grayscale, ignored }) => {
            const source: SourceCapture = JSON.parse(
                await readFile(
                    "fixtures/source/image-size-unavailable.json",
                    "utf8",
                ),
            );
            const fills = source.root.properties.fills as Record<
                string,
                unknown
            >[];
            fills[0].filters = filters;
            const before = JSON.stringify(source);
            const normalized = await normalizeSource(source);
            if (!normalized.ok || normalized.empty)
                throw Error("Missing snapshot");
            expect(
                normalized.warnings.some(
                    (w) => w.code === "IMAGE_FILTERS_IGNORED",
                ),
            ).toBe(ignored);
            const converted = convertSnapshot(normalized.snapshot);
            if (!converted.ok) throw Error("Missing Slint");
            expect(
                converted.source.includes("data:image/svg+xml;base64,"),
            ).toBe(grayscale);
            if (grayscale) {
                const data = converted.source.match(
                    /data:image\/svg\+xml;base64,([A-Za-z0-9+/=]+)/,
                )?.[1];
                if (!data) throw Error("Missing grayscale image");
                expect(Buffer.from(data, "base64").toString()).toContain(
                    '<feColorMatrix type="saturate" values="0"/>',
                );
            }
            expect(JSON.stringify(source)).toBe(before);
        },
    );
});

describe("image-tiling", () => {
    test.each([0.5, 1, 4])(
        "image tiles retain scale %s and use native repetition",
        async (scale) => {
            const source: SourceCapture = JSON.parse(
                await readFile(
                    "fixtures/source/image-size-unavailable.json",
                    "utf8",
                ),
            );
            const fills = source.root.properties.fills as Record<
                string,
                unknown
            >[];
            Object.assign(fills[0], {
                scaleMode: "TILE",
                scalingFactor: scale,
            });
            const before = JSON.stringify(source);
            const normalized = await normalizeSource(source);
            if (!normalized.ok || normalized.empty)
                throw Error("Missing snapshot");
            expect(normalized.snapshot.root).toMatchObject({
                fills: [{ scaleMode: "TILE", tileScale: scale }],
            });
            expect(
                normalized.warnings.some((w) =>
                    w.code.startsWith("IMAGE_TILE"),
                ),
            ).toBe(false);
            const converted = convertSnapshot(normalized.snapshot);
            if (!converted.ok) throw Error("Missing Slint");
            expect(converted.source).toContain("horizontal-tiling: repeat;");
            expect(converted.source).toContain("vertical-tiling: repeat;");
            expect(converted.source).not.toContain("image-fit: cover;");
            if (scale !== 1)
                expect(converted.source).toContain(
                    `transform-scale: ${scale};`,
                );
            expect(JSON.stringify(source)).toBe(before);
        },
    );

    test("invalid tile scale is diagnosed and defaults to intrinsic size", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile(
                "fixtures/source/image-size-unavailable.json",
                "utf8",
            ),
        );
        const fills = source.root.properties.fills as Record<string, unknown>[];
        Object.assign(fills[0], { scaleMode: "TILE", scalingFactor: 0 });
        const normalized = await normalizeSource(source);
        expect(normalized).toMatchObject({
            ok: true,
            snapshot: { root: { fills: [{ tileScale: 1 }] } },
        });
        if (!normalized.ok || normalized.empty) throw Error("Missing snapshot");
        expect(normalized.warnings).toContainEqual(
            expect.objectContaining({ code: "IMAGE_TILE_SCALE_APPROXIMATED" }),
        );
    });
});

describe("painted-bounds", () => {
    test("painted SVG bounds preserve the icon layout box and expand a directly selected vector", async () => {
        {
            const source = JSON.parse(
                await readFile("fixtures/source/painted-bounds.json", "utf8"),
            );
            const result = await normalizeSource(source);
            expect(result.ok).toBe(true);
            if (
                !result.ok ||
                result.empty ||
                !("children" in result.snapshot.root)
            )
                throw Error("Missing snapshot");
            const vector = result.snapshot.root.children[0];
            expect(vector.width).toBe(source.root.children[0].properties.width);
            expect(vector.x).toBe(source.root.children[0].properties.x);
            if (vector.kind !== "svg") throw Error("Missing SVG");
            expect(vector.paintBounds).toEqual(
                source.root.children[0].exports.svgBounds,
            );
            expect(vector.svg).toContain('stroke-linecap="round"');
            expect(vector.svg).toContain('stroke-linejoin="round"');
            expect(vector.raster).toBeUndefined();
            expect(
                result.warnings.some(
                    (warning) => warning.code === "PNG_EXPORT_FAILED",
                ),
            ).toBe(false);
            const selected = convertSnapshot({
                ...result.snapshot,
                root: vector,
            });
            expect(selected.ok).toBe(true);
            if (!selected.ok) throw Error("Conversion failed");
            expect(selected.width).toBe(vector.paintBounds?.width);
            expect(selected.height).toBe(vector.paintBounds?.height);
            const invalid = {
                ...result.snapshot,
                root: {
                    ...vector,
                    paintBounds: { x: 0, y: 0, width: -1, height: 10 },
                },
            };
            expect(validateSnapshot(invalid).ok).toBe(false);
        }
    });

    test("painted PNG fallback retains its offset when SVG is unavailable", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/painted-bounds.json", "utf8"),
        );
        source.root.children[0].exports = {
            svg: { error: "SVG export unavailable" },
            png: {
                value: Array.from(
                    await readFile("fixtures/authored/odd-size.png"),
                ),
            },
            rasterBounds: { x: -1, y: -1, width: 18.5, height: 18.5 },
        };
        const result = await normalizeSource(source);
        if (!result.ok || result.empty || !("children" in result.snapshot.root))
            throw Error("Missing snapshot");
        const vector = result.snapshot.root.children[0];
        if (vector.kind !== "svg") throw Error("Missing image");
        expect(vector.raster?.bounds).toEqual(
            source.root.children[0].exports.rasterBounds,
        );
        const converted = convertSnapshot(result.snapshot);
        if (!converted.ok) throw Error("Conversion failed");
        expect(converted.source).toContain("width: 18.5px;");
        expect(converted.source).toContain("x: -1px;");
    });
});

describe("viewer-image-width", () => {
    test("preview uses raster label width while export retains intrinsic sizing", async () => {
        const snapshot = JSON.parse(
            await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
        );
        const label = JSON.parse(
            await readFile("tests/non-inter-text.snapshot.json", "utf8"),
        );
        Object.assign(label.root, {
            width: 56,
            height: 24,
            layoutSizingHorizontal: "hug",
            layoutSizingVertical: "hug",
        });
        snapshot.root.children = [label.root];
        const preview = convertSnapshot(snapshot);
        const exported = convertSnapshot(snapshot, { target: "export" });
        if (!preview.ok || !exported.ok) throw Error("Conversion failed");
        expect(preview.source).toContain("width: 56px;");
        expect(preview.source).not.toContain("preferred-width: 56px;");
        expect(exported.source).toContain("preferred-width: 56px;");
    });
});

describe("mask-composition", () => {
    async function fixture(): Promise<SourceCapture> {
        return JSON.parse(
            await readFile("fixtures/source/rectangular-mask.json", "utf8"),
        );
    }
    test("rectangular masks retain native children and clip in local coordinates", async () => {
        const source = await fixture();
        const original = JSON.stringify(source);
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw Error("Expected masked preview");
        expect(result.snapshot.root).toMatchObject({
            children: [
                {
                    id: "mask",
                    kind: "frame",
                    x: 20,
                    y: 10,
                    width: 40,
                    height: 30,
                    clipsContent: true,
                    fills: [],
                    children: [
                        { id: "content", kind: "rectangle", x: -20, y: -10 },
                    ],
                },
            ],
        });
        expect(
            result.warnings.some((w) => w.code === "MASK_APPROXIMATED"),
        ).toBe(false);
        expect(convertSnapshot(result.snapshot).ok).toBe(true);
        expect(JSON.stringify(source)).toBe(original);
    });
    test("complex mask compositions use the containing PNG and never leak unmasked siblings", async () => {
        const source = await fixture();
        const mask = source.root.children?.[0];
        if (!mask) throw Error("Missing mask");
        mask.properties.maskType = "LUMINANCE";
        const image: SourceCapture = JSON.parse(
            await readFile(
                "fixtures/source/image-size-unavailable.json",
                "utf8",
            ),
        );
        source.pngEnabled = true;
        source.root.exports = {
            svg: {},
            png: { value: image.images["size-unavailable"].value?.bytes },
            rasterBounds: { x: -60, y: -30, width: 522, height: 1050 },
        };
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw Error("Expected PNG composition");
        expect(result.snapshot.root).toMatchObject({
            kind: "svg",
            sourceType: "MASK_COMPOSITION",
            raster: {
                bounds: { x: -60, y: -30, width: 522, height: 1050 },
            },
        });
        const converted = convertSnapshot(result.snapshot);
        expect(converted.ok).toBe(true);
        if (!converted.ok) throw Error("Expected conversion");
        expect(converted.source).toContain("width: 522px;");
        expect(converted.source).toContain("height: 1050px;");
        expect(result.warnings).toContainEqual(
            expect.objectContaining({
                code: "MASK_IMAGE_SUBSTITUTED",
                category: "image",
            }),
        );
        source.root.exports.png = { error: "Export unavailable" };
        const failed = await normalizeSource(source);
        expect(failed).toMatchObject({
            ok: false,
            diagnostics: expect.arrayContaining([
                expect.objectContaining({
                    code: "MASK_COMPOSITION_OMITTED",
                    category: "omission",
                }),
            ]),
        });
    });
});
