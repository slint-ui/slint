// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { packCaptureAssets, unpackCaptureAssets } from "../src/asset-transport";
import { isPluginToUiMessage } from "../src/protocol";
import { convertCapture } from "../src/preview/convert-capture";
import { packPreviewAssets, unpackPreviewAssets } from "../src/asset-transport";

describe("capture-assets", () => {
    test("binary capture transport preserves fixtures and conversion without mutating capture", async () => {
        for (const file of [
            "basic",
            "png-first",
            "image-size-unavailable",
            "multiple-failures",
        ]) {
            const source = JSON.parse(
                await readFile(`fixtures/source/${file}.json`, "utf8"),
            );
            const original = JSON.stringify(source);
            const packed = packCaptureAssets(source);
            expect(JSON.stringify(source)).toBe(original);
            expect(
                unpackCaptureAssets(packed.captureJson, packed.captureAssets),
            ).toEqual(source);
            const common = { type: "preview-capture" as const, revision: 2 };
            expect(isPluginToUiMessage({ ...common, ...packed })).toBe(true);
            const binary = await convertCapture({ ...common, ...packed });
            const json = await convertCapture({
                ...common,
                captureJson: original,
            });
            expect({ ...binary, trace: undefined }).toEqual({
                ...json,
                trace: undefined,
            });
            if (packed.captureAssets.length)
                expect(packed.captureJson.length).toBeLessThan(original.length);
        }
    });

    test("binary capture transport rejects invalid envelopes and dangling asset references", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/png-first.json", "utf8"),
        );
        const packed = packCaptureAssets(source);
        expect(packed.captureAssets.length).toBeGreaterThan(0);
        const common = { type: "preview-capture", revision: 2, ...packed };
        for (const bad of [
            { captureAssetVersion: 3 },
            { captureAssets: [[0, 1]] },
            { captureAssets: undefined },
        ])
            expect(isPluginToUiMessage({ ...common, ...bad })).toBe(false);
        expect(() => unpackCaptureAssets(packed.captureJson, [])).toThrow(
            "asset reference",
        );
        expect(
            await convertCapture({
                type: "preview-capture",
                revision: 2,
                ...packed,
                captureAssets: [],
            }),
        ).toMatchObject({
            type: "preview-diagnostics",
            trace: { outcome: "conversion-error" },
        });
    });

    test("cache sends references only for byte-identical assets and restores full data after reset", async () => {
        const { CaptureAssetSender, CaptureAssetReceiver } = await import(
            "../src/asset-transport"
        );
        const source = JSON.parse(
            await readFile("fixtures/source/png-first.json", "utf8"),
        );
        const sender = new CaptureAssetSender();
        const receiver = new CaptureAssetReceiver();
        const first = sender.pack(source);
        const firstBytes = receiver.resolve(first.captureAssets);
        const repeat = sender.pack(source);
        expect(
            repeat.captureAssets.every((asset) => typeof asset === "number"),
        ).toBe(true);
        expect(
            unpackCaptureAssets(
                repeat.captureJson,
                receiver.resolve(repeat.captureAssets),
            ),
        ).toEqual(source);
        const changed = structuredClone(source);
        const changePng = (node: typeof changed.root): boolean => {
            if (node.exports?.png?.value) {
                node.exports.png.value[node.exports.png.value.length - 1] ^= 1;
                return true;
            }
            return (node.children ?? []).some(changePng);
        };
        expect(changePng(changed.root)).toBe(true);
        const updated = sender.pack(changed);
        expect(
            updated.captureAssets.filter(
                (asset) => asset instanceof Uint8Array,
            ),
        ).toHaveLength(1);
        expect(
            unpackCaptureAssets(
                updated.captureJson,
                receiver.resolve(updated.captureAssets),
            ),
        ).toEqual(changed);
        // A restarted receiver can be repopulated with the UI's retained bytes.
        expect(new CaptureAssetReceiver().resolve(firstBytes)).toEqual(
            firstBytes,
        );
        expect(() =>
            new CaptureAssetReceiver().resolve(repeat.captureAssets),
        ).toThrow("Missing cached");
        sender.reset();
        expect(
            sender
                .pack(changed)
                .captureAssets.every((asset) => asset instanceof Uint8Array),
        ).toBe(true);
    });

    test("live replay retains immutable binary asset buffers through normalization", async () => {
        const { normalizeSource } = await import("../src/plugin/normalize");
        const { validateSource } = await import("../src/plugin/source");
        for (const fixture of ["png-first", "image-size-unavailable"]) {
            const source = JSON.parse(
                await readFile(`fixtures/source/${fixture}.json`, "utf8"),
            );
            const packed = packCaptureAssets(source);
            const originals = packed.captureAssets.map((asset) =>
                asset.slice(),
            );
            const binary = unpackCaptureAssets(
                packed.captureJson,
                packed.captureAssets,
                "binary",
            ) as import("../src/plugin/source").SourceCapture<
                import("../src/plugin/source").SourceBytes
            >;
            const retained: import("../src/plugin/source").SourceBytes[] = [];
            function visit(node: typeof binary.root) {
                if (node.exports?.png?.value)
                    retained.push(node.exports.png.value);
                for (const child of node.children ?? []) visit(child);
            }
            visit(binary.root);
            for (const image of Object.values(binary.images))
                if (image.value) retained.push(image.value.bytes);
            expect(retained.length).toBeGreaterThan(0);
            for (const bytes of retained)
                expect(
                    packed.captureAssets.some((asset) => asset === bytes),
                ).toBe(true);
            expect(() => validateSource(binary)).not.toThrow();
            expect(await normalizeSource(binary)).toEqual(
                await normalizeSource(source),
            );
            expect(packed.captureAssets).toEqual(originals);
            expect(
                unpackCaptureAssets(packed.captureJson, packed.captureAssets),
            ).toEqual(source);
        }
    });

    test("live binary capture packing retains the original image buffers", async () => {
        const { captureSelectionSource } = await import(
            "../src/plugin/capture"
        );
        const { CaptureAssetSender } = await import("../src/asset-transport");
        const bytes = new Uint8Array(
            await readFile("fixtures/authored/odd-size.png"),
        );
        const source = await captureSelectionSource(
            [
                {
                    id: "text",
                    name: "Text",
                    type: "VECTOR",
                    visible: true,
                } as SceneNode,
            ],
            Symbol("mixed"),
            async () => "",
            undefined,
            undefined,
            async () => bytes,
        );
        if (!source.ok || source.empty) throw Error("Expected capture");
        expect(source.source.root.exports?.png?.value).toBe(bytes);
        const packed = new CaptureAssetSender().pack(source.source);
        expect(packed.captureAssets[0]).toBe(bytes);
        const replay = unpackCaptureAssets(
            packed.captureJson,
            packed.captureAssets as Uint8Array[],
        );
        expect(
            (replay as { root: { exports: { png: { value: number[] } } } }).root
                .exports.png.value,
        ).toEqual(Array.from(bytes));
    });

    test("binary asset reuse checks complete words, trailing bytes and unaligned views", async () => {
        const { CaptureAssetSender } = await import("../src/asset-transport");
        for (const offset of [0, 1]) {
            const source = JSON.parse(
                await readFile("fixtures/source/png-first.json", "utf8"),
            );
            const original = new Uint8Array(11 + offset).subarray(offset);
            original.fill(42);
            source.root.exports.png.value = original;
            const sender = new CaptureAssetSender();
            sender.pack(source);
            source.root.exports.png.value = original.slice();
            expect(sender.pack(source).captureAssets[0]).toBe(0);
            for (const index of [4, 10]) {
                const changed = original.slice();
                changed[index]++;
                source.root.exports.png.value = changed;
                expect(sender.pack(source).captureAssets[0]).toBeInstanceOf(
                    Uint8Array,
                );
                source.root.exports.png.value = original;
                sender.pack(source);
            }
        }
    });
});

describe("preview-assets", () => {
    test("shares assets across source and snapshot without altering either string", () => {
        const data = Buffer.from("asset bytes".repeat(100)).toString("base64");
        const source = `Image { source: @image-url("data:image/png;base64,${data}"); }\nImage { source: @image-url("data:image/png;base64,${data}"); }`;
        const snapshotJson = JSON.stringify({
            fills: [
                { data, opacity: 0.25 },
                { data, opacity: 0.75 },
            ],
            text: '"data":"literal"',
        });
        const packed = packPreviewAssets(source, snapshotJson);
        expect(packed.assets).toEqual([data]);
        expect(unpackPreviewAssets(JSON.parse(JSON.stringify(packed)))).toEqual(
            {
                source,
                snapshotJson,
            },
        );
        expect(
            packPreviewAssets(source.replaceAll(data, "A".repeat(128)), "")
                .assets,
        ).toEqual(["A".repeat(128)]);
    });

    test("preserves empty, short and unusual literal content", () => {
        for (const source of [
            "",
            "data:image/png;base64,YQ==",
            '"data":"hello"',
            'text: "雪\\n";',
        ]) {
            expect(
                unpackPreviewAssets(packPreviewAssets(source, source)),
            ).toEqual({
                source,
                snapshotJson: source,
            });
        }
    });

    test("rejects malformed asset tables and references", () => {
        for (const value of [
            {
                assetVersion: 1,
                assets: new Array(1),
                source: [0],
                snapshotJson: [],
            },
            {
                assetVersion: 1,
                assets: [],
                source: new Array(1),
                snapshotJson: [],
            },
            null,
            {},
            { assetVersion: 2, assets: [], source: [], snapshotJson: [] },
            { assetVersion: 1, assets: [5], source: [], snapshotJson: [] },
            ...[-1, 0, 0.5, "bad"].map((reference) => ({
                assetVersion: 1,
                assets: [],
                source: reference === "bad" ? null : [reference],
                snapshotJson: [],
            })),
        ]) {
            expect(() => unpackPreviewAssets(value)).toThrow();
        }
    });
});

test("display and render programs share one asset table with validated references", () => {
    const data = "a".repeat(256);
    const source = `// readable\n@image-url("data:image/png;base64,${data}")`;
    const renderSource = `// specialized\n@image-url("data:image/png;base64,${data}")`;
    const packed = packPreviewAssets(source, "", renderSource);
    expect(packed.assets).toEqual([data]);
    expect(unpackPreviewAssets(packed)).toEqual({
        source,
        snapshotJson: "",
        renderSource,
    });
    expect(
        isPluginToUiMessage({
            type: "preview-source",
            revision: 1,
            source: packed,
        }),
    ).toBe(true);
    expect(
        isPluginToUiMessage({
            type: "preview-source",
            revision: 1,
            source: { ...packed, renderSource: [1] },
        }),
    ).toBe(false);
    expect(
        isPluginToUiMessage({
            type: "preview-source",
            revision: 1,
            source,
            renderSource: 4,
        }),
    ).toBe(false);
});
