// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test, vi } from "vitest";

import { readFile } from "node:fs/promises";
import { imageDimensions } from "../src/images";
import { captureSource } from "../src/plugin/capture";
import { normalizeSource } from "../src/plugin/normalize";
import { createSourceNormalizer } from "../src/plugin/normalize";
import { decodeValue, type SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import { bytesToBase64, utf8ToBase64 } from "../src/images";
import { NormalizationAssets } from "../src/images";
import { CaptureAssetReceiver } from "../src/asset-transport";
import {
    convertCapture,
    convertExport,
    type CaptureConversionState,
} from "../src/preview/convert-capture";

describe("image-dimensions", () => {
    async function fixture(): Promise<SourceCapture> {
        return JSON.parse(
            await readFile(
                "fixtures/source/image-size-unavailable.json",
                "utf8",
            ),
        );
    }

    test("missing Figma dimensions recover from saved image bytes without omitting the fill", async () => {
        const source = await fixture();
        const before = JSON.stringify(source);
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw Error("Expected image preview");
        expect(result.snapshot.root).toMatchObject({
            fills: [
                expect.objectContaining({
                    kind: "image",
                    intrinsicWidth: 2,
                    intrinsicHeight: 1,
                }),
            ],
        });
        expect(result.warnings).toContainEqual(
            expect.objectContaining({
                code: "IMAGE_DIMENSIONS_RECOVERED",
                nodeId: source.root.id,
            }),
        );
        expect(convertSnapshot(result.snapshot).ok).toBe(true);
        expect(JSON.stringify(source)).toBe(before);
    });

    test("capture keeps bytes and the size failure when the Figma size API rejects", async () => {
        const source = await fixture();
        const bytes = source.images["size-unavailable"].value?.bytes;
        if (!bytes) throw Error("Missing fixture bytes");
        const getBytesAsync = vi.fn(async () => new Uint8Array(bytes));
        vi.stubGlobal("figma", {
            getImageByHash: () => ({
                getBytesAsync,
                getSizeAsync: async () => {
                    throw Error("Image dimensions not available");
                },
            }),
        });
        try {
            const root = {
                ...decodeValue(source.root.properties),
                id: source.root.id,
                name: source.root.name,
                type: source.root.type,
            } as unknown as SceneNode;
            const captured = await captureSource(root, Symbol("mixed"));
            expect(captured.source.images).toEqual(source.images);
            expect(getBytesAsync).toHaveBeenCalledTimes(1);
        } finally {
            vi.unstubAllGlobals();
        }
    });

    test("GIF and JPEG dimensions are decoded and truncated headers are rejected", () => {
        expect(
            imageDimensions(
                new Uint8Array([71, 73, 70, 56, 57, 97, 20, 0, 10, 0, 0, 0, 0]),
            ),
        ).toEqual({ width: 20, height: 10 });
        const jpeg = new Uint8Array([
            255, 216, 255, 224, 0, 4, 0, 0, 255, 194, 0, 11, 8, 0, 10, 0, 20, 1,
            1, 17, 0,
        ]);
        expect(imageDimensions(jpeg)).toEqual({ width: 20, height: 10 });
        for (let length = 0; length < jpeg.length; length++)
            expect(imageDimensions(jpeg.slice(0, length))).toBeUndefined();
    });

    test("unrecoverable image headers warn and preserve other paints", async () => {
        const source = await fixture();
        source.images["size-unavailable"] = { value: { bytes: [1, 2, 3] } };
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw Error("Expected partial preview");
        expect(result.warnings).toContainEqual(
            expect.objectContaining({ code: "UNSUPPORTED_PAINT" }),
        );
    });

    test("shared image encoding keeps per-paint opacity and refreshes each normalization", async () => {
        const source = await fixture();
        const original = structuredClone(source.root);
        source.root = {
            ...source.root,
            id: "parent",
            type: "FRAME",
            properties: { ...source.root.properties, fills: [] },
            children: [
                { ...structuredClone(original), id: "first" },
                { ...structuredClone(original), id: "second" },
            ],
        };
        const first = source.root.children?.[0];
        const second = source.root.children?.[1];
        if (!first || !second) throw Error("Missing children");
        for (const [node, opacity] of [
            [first, 0.25],
            [second, 0.75],
        ] as const) {
            const fills = node.properties.fills as { [key: string]: unknown }[];
            fills[0].opacity = opacity;
        }
        const image = source.images["size-unavailable"].value;
        if (!image) throw Error("Missing image");
        const expected = Buffer.from(image.bytes).toString("base64");
        const result = await normalizeSource(source);
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    children: [
                        { fills: [{ data: expected, opacity: 0.25 }] },
                        { fills: [{ data: expected, opacity: 0.75 }] },
                    ],
                },
            },
        });
        image.bytes = [...(await readFile("fixtures/authored/square.png"))];
        const updated = await normalizeSource(source);
        expect(updated).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    children: [
                        {
                            fills: [
                                {
                                    data: Buffer.from(image.bytes).toString(
                                        "base64",
                                    ),
                                    opacity: 0.25,
                                },
                            ],
                        },
                        {
                            fills: [
                                {
                                    data: Buffer.from(image.bytes).toString(
                                        "base64",
                                    ),
                                    opacity: 0.75,
                                },
                            ],
                        },
                    ],
                },
            },
        });
    });
});

describe("encoding", () => {
    test("base64 matches the native reference across padding and chunk boundaries", () => {
        for (const length of [
            0, 1, 2, 3, 4, 255, 256, 257, 12287, 12288, 12289, 24575, 24576,
            24577, 1000001,
        ]) {
            const storage = Uint8Array.from(
                { length: length + 13 },
                (_, i) => (i * 73 + 19) & 255,
            );
            const bytes = storage.subarray(7, 7 + length);
            expect(bytesToBase64(bytes)).toBe(
                Buffer.from(bytes).toString("base64"),
            );
        }
    });

    test.each([
        "",
        "a",
        "ab",
        "abc",
        "\0\x7f",
        "\u0080\u07ff\u0800\uffff",
        "☃ 世界😀",
        ...[12287, 12288, 12289, 12290].map((size) => "a".repeat(size) + "😀"),
    ])(
        "encodes UTF-8 and base64 padding across chunk boundaries (%#)",
        (value) => {
            expect(utf8ToBase64(value)).toBe(
                Buffer.from(value).toString("base64"),
            );
        },
    );

    test("preserves existing isolated-surrogate encoding", () => {
        expect(utf8ToBase64("\ud800")).toBe("7aCA");
        expect(utf8ToBase64("\udfff")).toBe("7b+/");
    });
});

describe("normalization-assets", () => {
    test("immutable bytes encode once; distinct byte objects do not share contextual pixels", () => {
        const assets = new NormalizationAssets();
        const input = [1, 2, 3];
        const bytes = assets.bytes(input);
        expect(assets.bytes(input)).toBe(bytes);
        expect(assets.encode(bytes)).toBe("AQID");
        expect(assets.encode(bytes)).toBe("AQID");
        expect(assets.encodings).toBe(1);
        assets.encode(new Uint8Array([1, 2, 3]));
        expect(assets.encodings).toBe(2);
    });
    test("retained capture shares parsed source and asset encoding across preview and native export", async () => {
        const request = {
            type: "preview-capture" as const,
            revision: 1,
            captureJson: await readFile(
                "fixtures/source/export-fonts.json",
                "utf8",
            ),
        };
        const state: CaptureConversionState = {};
        const receiver = new CaptureAssetReceiver();
        const preview = await convertCapture(request, receiver, state);
        expect(preview.type).toBe("preview-source");
        const source = state.source,
            normalizer = state.normalizer;
        const encodings = normalizer?.assets.encodings;
        expect(encodings).toBeGreaterThan(0);
        const native = await convertExport(request, receiver, state);
        expect(native.source).toContain("Native export text");
        expect(state.source).toBe(source);
        expect(state.normalizer).toBe(normalizer);
        expect(normalizer?.assets.encodings).toBe(encodings);
    });
    test("shared target normalization preserves snapshot and warnings, public boundary rejects mutated input", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/component-variants.json", "utf8"),
        ) as SourceCapture;
        const retained = createSourceNormalizer(source);
        for (const target of ["preview", "export"] as const) {
            const actual = await retained.normalize(target);
            const standalone = await normalizeSource(source, target);
            expect(actual.ok).toBe(standalone.ok);
            if (
                !actual.ok ||
                actual.empty ||
                !standalone.ok ||
                standalone.empty
            )
                throw Error("normalization failed");
            expect(actual.snapshot).toEqual(standalone.snapshot);
            expect(actual.warnings).toEqual(standalone.warnings);
        }
        source.root.properties.width = Infinity;
        await expect(normalizeSource(source)).rejects.toThrow("Invalid");
    });
});
