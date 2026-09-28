// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:words opsz

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
import { validateSnapshot } from "../src/plugin/snapshot";

describe("source", () => {
    test("captures text paint bounds without changing the Figma layout box", async () => {
        const fixture: SourceCapture = JSON.parse(
            await readFile("fixtures/source/export-fonts.json", "utf8"),
        );
        const label = fixture.root.children?.[0];
        if (!label) throw Error("Expected text fixture");
        const node = {
            ...decodeValue(label.properties),
            id: label.id,
            name: label.name,
            type: "TEXT",
            fontName: { family: "Inter", style: "Regular" },
            width: 24,
            height: 24,
            absoluteTransform: [
                [1, 0, 100],
                [0, 1, 200],
            ],
            absoluteBoundingBox: { x: 100, y: 200, width: 24, height: 24 },
            absoluteRenderBounds: { x: 97, y: 197, width: 30, height: 30 },
            getStyledTextSegments: () => [],
        } as unknown as TextNode;
        const captured = await captureSource(node, SOURCE_MIXED);
        expect(
            decodeValue(captured.source.root.properties.textPaintBounds),
        ).toEqual({ x: -3, y: -3, width: 30, height: 30 });
        expect(captured.source.root.properties.width).toBe(24);
        expect(captured.source.root.properties.height).toBe(24);
        const rotated = {
            ...node,
            absoluteTransform: [
                [0, -1, 100],
                [1, 0, 200],
            ],
        } as unknown as TextNode;
        const skipped = await captureSource(rotated, SOURCE_MIXED);
        expect(skipped.source.root.properties.textPaintBounds).toBeUndefined();
    });

    test.each(["Inter", "Roboto", "Material Symbols Outlined"])(
        "wraps overflowing text without font-specific sizing for %s",
        async (family) => {
            const source: SourceCapture = JSON.parse(
                await readFile("fixtures/source/export-fonts.json", "utf8"),
            );
            const label = source.root.children?.[0];
            if (!label) throw Error("Expected text fixture");
            source.root = label;
            Object.assign(label.properties, {
                characters:
                    family === "Material Symbols Outlined"
                        ? "check_circle"
                        : "W",
                fontName: encodeValue({ family, style: "Regular" }),
                fontSize: 30,
                width: 24,
                height: 24,
                textAutoResize: "NONE",
                textAlignHorizontal: "CENTER",
                textAlignVertical: "CENTER",
                textPaintBounds: { x: -3, y: -3, width: 30, height: 30 },
            });
            const before = structuredClone(source);
            const normalized = await normalizeSource(source, "export");
            if (
                !normalized.ok ||
                normalized.empty ||
                normalized.snapshot.root.kind !== "text"
            )
                throw Error("Expected text snapshot");
            const snapshot = {
                ...normalized.snapshot,
                root: normalized.snapshot.root,
            };
            expect(snapshot.root).toMatchObject({
                width: 24,
                height: 24,
                paintBounds: { x: -3, y: -3, width: 30, height: 30 },
            });
            expect(validateSnapshot(snapshot).ok).toBe(true);
            const converted = convertSnapshot(snapshot, { target: "export" });
            if (!converted.ok) throw Error("Expected native source");
            expect(converted.source).toContain(
                "width: parent.width + 2px * ceil(max(0px, self.preferred-width - parent.width) / 2px + 3);",
            );
            expect(converted.source).toContain(
                "height: parent.height + 2px * ceil(max(0px, self.preferred-height - parent.height) / 2px + 3);",
            );
            expect(converted.source).toContain(
                "x: (parent.width - self.width) / 2;",
            );
            expect(converted.source).toContain(
                "y: (parent.height - self.height) / 2;",
            );
            expect(converted.source).toContain("wrap: word-wrap;");
            const single = converted.source.match(
                /single := Text \{([\s\S]*?)\n\s+\}/,
            )?.[1];
            expect(single).toBeDefined();
            expect(single).not.toContain("wrap: word-wrap;");
            expect(converted.source).not.toContain("overflow: visible;");
            expect(source).toEqual(before);
            const bound = convertSnapshot(snapshot, {
                target: "export",
                scope: "root-only",
                codegenVariables: [
                    {
                        field: "fontSize",
                        name: "Text Size",
                        collection: "Typography",
                        modes: 1,
                        type: "FLOAT",
                    },
                ],
            });
            if (!bound.ok) throw Error("Expected variable source");
            expect(bound.source).toContain("font-size: typography.text-size;");
            expect(
                bound.source.match(/font-size: typography.text-size;/g),
            ).toHaveLength(3);
            expect(
                bound.warnings.some(
                    (warning) => warning.code === "CODEGEN_VARIABLE_FALLBACK",
                ),
            ).toBe(false);
            for (const root of [
                { ...snapshot.root, characters: "Two words" },
                { ...snapshot.root, characters: "Two\nlines" },
                {
                    ...snapshot.root,
                    paintBounds: { x: -3, y: -3, width: 30, height: 60 },
                },
                { ...snapshot.root, overflow: "elide" as const },
            ]) {
                const unchanged = convertSnapshot(
                    {
                        ...snapshot,
                        root: {
                            ...root,
                            runs: root.runs.map((run) => ({
                                ...run,
                                range: [0, root.characters.length] as const,
                                text: root.characters,
                            })),
                        },
                    },
                    { target: "export" },
                );
                if (!unchanged.ok) throw Error("Expected text source");
                expect(
                    unchanged.source.includes(
                        "self.preferred-width - parent.width",
                    ),
                ).toBe(root.overflow !== "elide");
            }
            for (const horizontalAlign of ["LEFT", "RIGHT"] as const) {
                const aligned = convertSnapshot(
                    {
                        ...snapshot,
                        root: {
                            ...snapshot.root,
                            horizontalAlign,
                            verticalAlign:
                                horizontalAlign === "LEFT" ? "TOP" : "BOTTOM",
                        },
                    },
                    { target: "export" },
                );
                if (!aligned.ok) throw Error("Expected aligned source");
                expect(aligned.source).toContain(
                    horizontalAlign === "LEFT"
                        ? "x: (self.preferred-width - self.width) / 2;"
                        : "x: parent.width - (self.width + self.preferred-width) / 2;",
                );
                expect(aligned.source).toContain(
                    horizontalAlign === "LEFT"
                        ? "y: (self.preferred-height - self.height) / 2;"
                        : "y: parent.height - (self.height + self.preferred-height) / 2;",
                );
            }
            expect(
                validateSnapshot({
                    ...snapshot,
                    root: {
                        ...snapshot.root,
                        paintBounds: { x: 0, y: 0, width: -1, height: 30 },
                    },
                }).ok,
            ).toBe(false);
        },
    );

    test.each(["Inter", "Roboto", "Material Symbols Outlined"])(
        "preserves text geometry and wrapping for %s",
        async (family) => {
            const source: SourceCapture = JSON.parse(
                await readFile("fixtures/source/export-fonts.json", "utf8"),
            );
            const label = source.root.children?.[0];
            if (!label) throw Error("Expected text fixture");
            source.root = label;
            Object.assign(label.properties, {
                characters: "Wide text",
                fontName: encodeValue({
                    family,
                    style: "Regular",
                }),
                fontSize: 30,
                width: 24,
                height: 24,
                textAutoResize: "NONE",
                textAlignHorizontal: "CENTER",
                textAlignVertical: "CENTER",
            });
            const result = await normalizeSource(source, "export");
            if (!result.ok || result.empty)
                throw Error("Expected text snapshot");
            expect(result.snapshot.root).toMatchObject({
                width: 24,
                height: 24,
            });
            const converted = convertSnapshot(result.snapshot, {
                target: "export",
            });
            if (!converted.ok) throw Error("Expected native source");
            expect(converted.source).not.toContain("max(parent.width");
            expect(converted.source).toContain("wrap: word-wrap;");
            expect(converted.source).toContain("width: 24px;");
            expect(converted.source).toContain("height: 24px;");
            const bound = convertSnapshot(result.snapshot, {
                target: "export",
                scope: "root-only",
                codegenVariables: [
                    {
                        field: "fontSize",
                        name: "Icon Size",
                        collection: "Icons",
                        modes: 1,
                        type: "FLOAT",
                    },
                ],
            });
            if (!bound.ok) throw Error("Expected bound icon source");
            expect(bound.source).toContain("font-size: icons.icon-size;");
            expect(
                bound.warnings.some(
                    (w) => w.code === "CODEGEN_VARIABLE_FALLBACK",
                ),
            ).toBe(false);
        },
    );

    test.each([
        [{ wght: 650, ital: 1 }, 650, true],
        [{ wght: 350, ital: 0 }, 350, false],
    ])(
        "exports supported font axes %j as native typography",
        async (axes, weight, italic) => {
            const source: SourceCapture = JSON.parse(
                await readFile("fixtures/source/export-fonts.json", "utf8"),
            );
            const label = source.root.children?.[0];
            if (!label) throw Error("Expected text fixture");
            source.root = label;
            source.root.properties.fontName = encodeValue({
                family: "Roboto",
                style: "Regular",
                variationSettings: axes,
            });
            const before = structuredClone(source);
            const result = await normalizeSource(source, "export");
            expect(result.ok).toBe(true);
            if (!result.ok || result.empty)
                throw Error("Expected text snapshot");
            expect(result.snapshot.root).toMatchObject({
                kind: "text",
                fontVariationSettings: axes,
                fontWeight: weight,
                italic,
                runs: [{ bold: weight >= 600, italic }],
            });
            expect(validateSnapshot(result.snapshot).ok).toBe(true);
            const converted = convertSnapshot(result.snapshot);
            if (!converted.ok) throw Error("Expected native source");
            expect(converted.source).toContain(`font-weight: ${weight};`);
            expect(converted.source.includes("font-italic: true;")).toBe(
                italic,
            );
            expect(result.warnings).toEqual([]);
            expect(source).toEqual(before);
        },
    );

    test("retains unsupported Material Symbols axes and reports the native limitation", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/export-fonts.json", "utf8"),
        );
        const label = source.root.children?.[0];
        if (!label) throw Error("Expected text fixture");
        source.root = label;
        const axes = { FILL: 1, GRAD: 0, opsz: 24, wght: 400 };
        source.root.properties.fontName = encodeValue({
            family: "Material Symbols Outlined",
            style: "Regular",
            variationSettings: axes,
        });
        const result = await normalizeSource(source, "export");
        if (!result.ok || result.empty) throw Error("Expected text snapshot");
        expect(result.snapshot.root).toMatchObject({
            fontVariationSettings: axes,
        });
        expect(result.warnings).toEqual([
            expect.objectContaining({
                code: "FONT_VARIATIONS_APPROXIMATED",
                message: expect.stringContaining("FILL=1, GRAD=0, opsz=24"),
            }),
        ]);
    });

    test.each([{ bad: 1 }, { wght: "bold" }, null, []])(
        "rejects malformed font axes %j",
        async (axes) => {
            const source: SourceCapture = JSON.parse(
                await readFile("fixtures/source/export-fonts.json", "utf8"),
            );
            const label = source.root.children?.[0];
            if (!label) throw Error("Expected text fixture");
            source.root = label;
            source.root.properties.fontName = encodeValue({
                family: "Roboto",
                style: "Regular",
                variationSettings: axes,
            });
            expect(await normalizeSource(source, "export")).toMatchObject({
                ok: false,
                diagnostics: [
                    expect.objectContaining({
                        code: "INVALID_FONT_VARIATIONS",
                    }),
                ],
            });
        },
    );

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
                undefined,
                undefined,
                planned ? "tree" : "flattened",
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
            validateSnapshot({
                ...badSideWeight,
                schemaVersion: 8,
                selection: { nodeId: "x", nodeName: "x" },
            }).ok,
        ).toBe(false);
        const badShadow = structuredClone(fixture);
        badShadow.root.children[0].shadows[0].spread = "wide";
        expect(
            validateSnapshot({
                ...badShadow,
                schemaVersion: 8,
                selection: { nodeId: "x", nodeName: "x" },
            }).ok,
        ).toBe(false);
        const negativeSpread = structuredClone(fixture);
        negativeSpread.root.children[0].shadows[0].spread = -2;
        expect(
            validateSnapshot({
                ...negativeSpread,
                schemaVersion: 8,
                selection: { nodeId: "x", nodeName: "x" },
            }).ok,
        ).toBe(true);
        const badRotation = structuredClone(fixture);
        badRotation.root.rotation = Number.NaN;
        expect(
            validateSnapshot({
                ...badRotation,
                schemaVersion: 8,
                selection: { nodeId: "x", nodeName: "x" },
            }).ok,
        ).toBe(false);
        const badRun = structuredClone(fixture);
        const badRunNode = badRun.root.children[0].children[1] as unknown as {
            runs: Array<{ range: number[] }>;
        };
        badRunNode.runs[0].range = [0, 999];
        expect(
            validateSnapshot({
                ...badRun,
                schemaVersion: 8,
                selection: { nodeId: "x", nodeName: "x" },
            }).ok,
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
