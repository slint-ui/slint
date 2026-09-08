// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { captureSource } from "../src/plugin/capture";
import { readFile } from "node:fs/promises";
import { normalizeSource } from "../src/plugin/normalize";
import {
    SOURCE_MIXED,
    decodeValue,
    encodeValue,
    sourceImageHashes,
    type SourceCapture,
    type SourceNode,
} from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import { uniqueDiagnostics, warningSummaries } from "../src/plugin/snapshot";
import type { Diagnostic } from "../src/plugin/snapshot";
import { parseSnapshot } from "../src/plugin/snapshot";

describe("source", () => {
    test("saved source replays deterministically without a Figma host", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/basic.json", "utf8"),
        );
        const first = await normalizeSource(source);
        const second = await normalizeSource(
            JSON.parse(JSON.stringify(source)),
        );
        expect(second).toEqual(first);
        expect(first.ok).toBe(true);
        if (!first.ok || first.empty) throw new Error("Expected snapshot");
        expect(convertSnapshot(first.snapshot).ok).toBe(true);
        expect(first.snapshot.root).toMatchObject({
            kind: "rectangle",
            width: 100,
            height: 60,
        });
    });

    test("image metadata scanning finds nested paints once without changing source data", () => {
        const metadata = {
            fills: [{ imageHash: "a" }, { imageHash: null }],
            segments: [{ fills: [{ imageHash: "b" }, { imageHash: "a" }] }],
            fontSize: 12,
            visible: true,
            optional: undefined,
        };
        const before = structuredClone(metadata);
        expect([...sourceImageHashes(metadata)]).toEqual(["a", "b"]);
        expect(metadata).toEqual(before);
    });

    test("mask planning skips covered exports, keeps metadata and preserves normalized output", async () => {
        const fixture: SourceCapture = JSON.parse(
            await readFile("fixtures/source/rectangular-mask.json", "utf8"),
        );
        const png = new Uint8Array(
            await readFile("fixtures/authored/odd-size.png"),
        );
        const mask = fixture.root.children?.[0];
        const content = fixture.root.children?.[1];
        if (!mask || !content) throw Error("Missing mask fixture children");
        content.type = "VECTOR";
        const materialize = (node: SourceNode): SceneNode =>
            ({
                ...decodeValue(node.properties),
                id: node.id,
                name: node.name,
                type: node.type,
                ...(node.children
                    ? { children: node.children.map(materialize) }
                    : {}),
            }) as SceneNode;
        const capture = async (planned: boolean, fail = false) => {
            const calls: string[] = [];
            const result = await captureSource(
                materialize(fixture.root),
                SOURCE_MIXED,
                async () => "",
                async () => undefined,
                async (node) => {
                    calls.push(node.id);
                    if (fail) throw Error("Export unavailable");
                    return png;
                },
                2,
                false,
                undefined,
                undefined,
                4,
                false,
                undefined,
                undefined,
                planned,
            );
            return { calls, source: result.source };
        };
        // Native rectangular clipping must still export the vector it contains.
        expect((await capture(true)).calls).toEqual([content.id]);
        mask.properties.maskType = "LUMINANCE";
        for (const fail of [false, true]) {
            const baseline = await capture(false, fail);
            const planned = await capture(true, fail);
            expect(baseline.calls).toEqual([content.id, fixture.root.id]);
            expect(planned.calls).toEqual([fixture.root.id]);
            expect(
                planned.source.root.children?.map((n) => n.properties),
            ).toEqual(baseline.source.root.children?.map((n) => n.properties));
            const before = await normalizeSource(baseline.source);
            const after = await normalizeSource(planned.source);
            // Capture metrics contain export counts; compare render contract and diagnostics.
            expect({ ...after, captureMetrics: undefined }).toEqual({
                ...before,
                captureMetrics: undefined,
            });
            if (after.ok && !after.empty && before.ok && !before.empty)
                expect(convertSnapshot(after.snapshot)).toEqual(
                    convertSnapshot(before.snapshot),
                );
        }
        mask.properties.visible = false;
        expect((await capture(true)).calls).toEqual([content.id]);
    });

    test("source encoding preserves values the render contract cannot accept", () => {
        const raw = {
            gap: -2,
            mixed: SOURCE_MIXED,
            absent: undefined,
            nan: Number.NaN,
            infinite: Number.POSITIVE_INFINITY,
        };
        expect(
            decodeValue(JSON.parse(JSON.stringify(encodeValue(raw)))),
        ).toEqual(raw);
    });

    test("malformed source envelopes are rejected before replay", async () => {
        await expect(
            normalizeSource({ sourceVersion: 2 } as unknown as SourceCapture),
        ).rejects.toThrow("Invalid source");
    });

    test("capture retains property failures and original values before normalization", async () => {
        const node = {
            id: "capture:1",
            name: "Unreadable corner",
            type: "RECTANGLE",
            visible: true,
            itemSpacing: -2,
            get cornerRadius() {
                throw new Error("Property unavailable");
            },
        } as unknown as SceneNode;
        const result = await captureSource(node, Symbol("mixed"));
        expect(result.source.root.properties.itemSpacing).toBe(-2);
        expect(result.source.root.errors).toEqual([
            { property: "cornerRadius", message: "Property unavailable" },
        ]);
        expect(JSON.parse(JSON.stringify(result.source))).toEqual(
            result.source,
        );
    });

    test("PNG-first JSON replays without false export warnings and preserves generated code", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/png-first.json", "utf8"),
        );
        const first = await normalizeSource(source);
        const baseline = structuredClone(source);
        if (!baseline.root.exports) throw Error("Missing exports");
        baseline.root.exports.svgOmitted = undefined;
        baseline.root.exports.svg = {
            value: '<svg viewBox="0 0 24 24"><path /></svg>',
        };
        const previous = await normalizeSource(baseline);
        if (!first.ok || first.empty || !previous.ok || previous.empty)
            throw Error("Expected snapshots");
        expect(first.warnings).toEqual(previous.warnings);
        expect(
            first.warnings.some((w) => w.code === "SVG_EXPORT_FALLBACK"),
        ).toBe(false);
        expect(convertSnapshot(first.snapshot)).toEqual(
            convertSnapshot(previous.snapshot),
        );
        expect(
            await normalizeSource(JSON.parse(JSON.stringify(source))),
        ).toEqual(first);
        baseline.root.exports.svg = { error: "Actual SVG failure" };
        const failure = await normalizeSource(baseline);
        expect(
            failure.ok &&
                !failure.empty &&
                failure.warnings.some((w) => w.code === "SVG_EXPORT_FALLBACK"),
        ).toBe(true);
    });

    test("SVG omission requires valid PNG provenance", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/png-first.json", "utf8"),
        );
        for (const png of [
            undefined,
            { value: [] },
            { value: [1, 2, 3] },
            {
                value: source.root.exports?.png?.value,
                error: "Conflicting PNG error",
            },
        ]) {
            const invalid = structuredClone(source);
            if (!invalid.root.exports) throw Error("Missing exports");
            invalid.root.exports.png = png;
            await expect(normalizeSource(invalid)).rejects.toThrow(
                "Invalid or duplicate source node",
            );
        }
    });

    test("masked parent PNG exports retain painted bounds outside the layout box", async () => {
        const node = {
            id: "popover",
            name: "Popover",
            type: "FRAME",
            visible: true,
            width: 402,
            height: 992,
            absoluteTransform: [
                [1, 0, 1000],
                [0, 1, 100],
            ],
            absoluteBoundingBox: { x: 1000, y: 100, width: 402, height: 992 },
            absoluteRenderBounds: { x: 940, y: 70, width: 522, height: 1050 },
            children: [
                {
                    id: "mask",
                    name: "Mask",
                    type: "RECTANGLE",
                    visible: true,
                    isMask: true,
                    maskType: "LUMINANCE",
                },
            ],
        } as unknown as SceneNode;
        const png = new Uint8Array(
            await readFile("fixtures/authored/odd-size.png"),
        );
        const result = await captureSource(
            node,
            Symbol("mixed"),
            async () => "",
            async () => undefined,
            async () => png,
            2,
        );
        expect(result.source.root.exports?.rasterBounds).toEqual({
            x: -60,
            y: -30,
            width: 522,
            height: 1050,
        });
    });

    test("PNG-only capture preserves export errors without SVG fallback", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/png-first.json", "utf8"),
        );
        if (!source.root.exports) throw Error("Missing exports");
        source.root.exports.png = { error: "PNG failed" };
        expect(await normalizeSource(source)).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({
                    code: "PNG_EXPORT_FAILED",
                    message: expect.stringContaining("PNG failed"),
                }),
            ],
        });
    });
});

describe("diagnostics", () => {
    test("deduplication preserves first occurrences and computes each identity once", () => {
        const warnings: Diagnostic[] = Array.from(
            { length: 2_000 },
            (_, i) => ({
                severity: "warning",
                code: "GEOMETRY_APPROXIMATED",
                message: "Captured geometry",
                nodeId: String(i),
            }),
        );
        const input = [
            ...warnings,
            ...warnings.map((warning) => ({ ...warning })),
        ];
        let calls = 0;
        const result = uniqueDiagnostics(input, (warning) => {
            calls++;
            return JSON.stringify(warning);
        });
        expect(calls).toBe(input.length);
        expect(result).toEqual(warnings);
        expect(result[0]).toBe(warnings[0]);
        expect(
            uniqueDiagnostics([
                warnings[0],
                { ...warnings[0], message: "Different fallback" },
            ]),
        ).toHaveLength(2);
    });

    test("document warnings ignore node metadata but retain distinct limitations and stable order", () => {
        const first: Diagnostic = {
            severity: "warning",
            code: "BLEND_MODE_APPROXIMATED",
            message:
                "Non-normal blend modes are rendered with normal compositing",
            category: "approximation",
            nodeId: "one",
            nodePath: "Frame / One",
            originalValue: "MULTIPLY",
        };
        const warnings = [
            first,
            {
                ...first,
                nodeId: "two",
                nodePath: "Frame / Two",
                originalValue: "SCREEN",
            },
            { ...first, message: "A different fallback" },
            { ...first, code: "OTHER_LIMITATION" },
            first,
        ];
        const before = JSON.stringify(warnings);
        expect(warningSummaries(warnings)).toEqual([
            {
                code: first.code,
                message: first.message,
                category: "approximation",
            },
            {
                code: first.code,
                message: "A different fallback",
                category: "approximation",
            },
            {
                code: "OTHER_LIMITATION",
                message: first.message,
                category: "approximation",
            },
        ]);
        expect(JSON.stringify(warnings)).toBe(before);
        expect(warningSummaries([])).toEqual([]);
        expect(warningSummaries([first])).toHaveLength(1);
    });

    test("node-specific component messages become one generic limitation", () => {
        const warnings: Diagnostic[] = ["Button", "Checkbox"].map((name) => ({
            severity: "warning",
            code: "COMPONENT_PROPERTY_STATIC",
            nodeId: name,
            message: `${name}.Label: the authored property has no editable native binding in this capture (for example, rasterized text)`,
        }));
        expect(warningSummaries(warnings)).toEqual([
            {
                code: "COMPONENT_PROPERTY_STATIC",
                message:
                    "Some authored properties have no editable native binding in this capture (for example, rasterized text)",
                category: undefined,
            },
        ]);
        expect(
            warningSummaries([
                {
                    ...warnings[0],
                    code: "constructor",
                    message: "Unknown warning",
                },
            ])[0].message,
        ).toBe("Unknown warning");
    });
});

describe("milestone7", () => {
    test("rejects malformed resilient snapshot fields deterministically", async () => {
        const json = await readFile(
            "fixtures/milestone7-resilience.snapshot.json",
            "utf8",
        );
        const fixture = JSON.parse(json) as {
            root: {
                rotation: number;
                children: Array<{
                    strokes: Array<Record<string, unknown>>;
                    shadows: Array<Record<string, unknown>>;
                    children: Array<Record<string, unknown>>;
                }>;
            };
        };
        const badSideWeight = structuredClone(fixture);
        badSideWeight.root.children[0].strokes[0].strokeRightWeight = -1;
        expect(
            parseSnapshot(
                JSON.stringify({
                    ...badSideWeight,
                    schemaVersion: 8,
                    selection: { nodeId: "x", nodeName: "x" },
                }),
            ).ok,
        ).toBe(false);
        const badShadow = structuredClone(fixture);
        badShadow.root.children[0].shadows[0].spread = "wide";
        expect(
            parseSnapshot(
                JSON.stringify({
                    ...badShadow,
                    schemaVersion: 8,
                    selection: { nodeId: "x", nodeName: "x" },
                }),
            ).ok,
        ).toBe(false);
        const negativeSpread = structuredClone(fixture);
        negativeSpread.root.children[0].shadows[0].spread = -2;
        expect(
            parseSnapshot(
                JSON.stringify({
                    ...negativeSpread,
                    schemaVersion: 8,
                    selection: { nodeId: "x", nodeName: "x" },
                }),
            ).ok,
        ).toBe(true);
        const badRotation = structuredClone(fixture);
        badRotation.root.rotation = Number.NaN;
        expect(
            parseSnapshot(
                JSON.stringify({
                    ...badRotation,
                    schemaVersion: 8,
                    selection: { nodeId: "x", nodeName: "x" },
                }),
            ).ok,
        ).toBe(false);
        const badRun = structuredClone(fixture);
        const badRunNode = badRun.root.children[0].children[1] as unknown as {
            runs: Array<{ range: number[] }>;
        };
        badRunNode.runs[0].range = [0, 999];
        expect(
            parseSnapshot(
                JSON.stringify({
                    ...badRun,
                    schemaVersion: 8,
                    selection: { nodeId: "x", nodeName: "x" },
                }),
            ).ok,
        ).toBe(false);
    });
});

// Replay regressions for the shared table measurement and visual-export paths.
test("captured table cells determine both root bounds and cell positions", async () => {
    const source: SourceCapture = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    source.root.type = "TABLE";
    delete source.root.properties.width;
    delete source.root.properties.height;
    Object.assign(source.root.properties, { numRows: 2, numColumns: 2 });
    source.root.cells = [
        [10, 5],
        [30, 10],
        [15, 25],
        [20, 20],
    ].map(([width, height]) => ({ width, height }));
    const before = structuredClone(source);
    for (const target of ["preview", "export"] as const) {
        const result = await normalizeSource(source, target);
        if (!result.ok || result.empty) throw Error("Expected table snapshot");
        expect(result.snapshot.root).toMatchObject({
            width: 45,
            height: 35,
            children: [
                { x: 0, y: 0, width: 10, height: 5 },
                { x: 15, y: 0, width: 30, height: 10 },
                { x: 0, y: 10, width: 15, height: 25 },
                { x: 15, y: 10, width: 20, height: 20 },
            ],
        });
    }
    expect(source).toEqual(before);
    source.root.cells[2] = { width: -1, height: 25 };
    expect(await normalizeSource(source)).toMatchObject({
        ok: false,
        diagnostics: [
            {
                code: "INVALID_TABLE_CELL_GEOMETRY",
                propertyPath: "cellAt(1,0)",
            },
        ],
    });
});

test.each(["TEXT", "VECTOR"])(
    "captured %s exports retain their SVG fallback policy",
    async (type) => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/export-fonts.json", "utf8"),
        );
        const label = source.root.children?.[0];
        if (!label) throw Error("Expected captured label");
        source.root = label;
        source.root.type = type;
        source.pngEnabled = false;
        source.root.exports = {
            svg: {
                value: '<svg xmlns="http://www.w3.org/2000/svg"><text>Label</text></svg>',
            },
        };
        const result = await normalizeSource(source);
        if (type === "TEXT") {
            expect(result).toMatchObject({
                ok: false,
                diagnostics: [{ code: "TEXT_SVG_EXPORT_INVALID" }],
            });
            expect(await normalizeSource(source, "export")).toMatchObject({
                ok: true,
                snapshot: { root: { kind: "text" } },
            });
        } else
            expect(result).toMatchObject({
                ok: true,
                snapshot: { root: { kind: "svg" } },
            });
        source.pngEnabled = true;
        source.root.exports.png = { error: "captured PNG failed" };
        expect(await normalizeSource(source)).toMatchObject({
            ok: false,
            diagnostics: [
                {
                    code: "PNG_EXPORT_FAILED",
                    message: "Failed to export PNG: captured PNG failed",
                },
            ],
        });
    },
);
