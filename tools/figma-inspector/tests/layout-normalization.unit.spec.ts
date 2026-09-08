// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { normalizeSource } from "../src/plugin/normalize";
import type { SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import type { SnapshotNode } from "../src/plugin/snapshot";

describe("layout-normalization", () => {
    async function layout(): Promise<SourceCapture> {
        return JSON.parse(
            await readFile("fixtures/source/negative-gap.json", "utf8"),
        );
    }

    test("negative gap source stays intact while native flex layout preserves overlap", async () => {
        const source = await layout();
        const result = await normalizeSource(source);
        expect(source.root.properties.itemSpacing).toBe(-2);
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    autoLayout: { itemSpacing: -2 },
                    layoutFallback: "flex",
                },
            },
            warnings: [],
        });
        if (!result.ok || result.empty) throw new Error("Expected snapshot");
        expect(convertSnapshot(result.snapshot).ok).toBe(true);
    });

    test("unsupported alignment preserves child flex layouts within positioned parent", async () => {
        const source = await layout();
        const child = structuredClone(source.root);
        child.id = "nested";
        source.root.children = [child];
        source.root.properties.primaryAxisAlignItems = "NEW_ALIGNMENT";
        const result = await normalizeSource(source);
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    autoLayout: null,
                    layoutFallback: "freeform",
                    children: [{ autoLayout: { direction: "vertical" } }],
                },
            },
        });
    });

    test("wrapped gaps, padding and sizing report independent substitutions", async () => {
        const source = await layout();
        Object.assign(source.root.properties, {
            layoutWrap: "WRAP",
            counterAxisSpacing: -5,
            paddingTop: { $source: "NaN" },
            layoutSizingVertical: "NEW_SIZE",
        });
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw new Error("Expected snapshot");
        expect(result.warnings.map((w) => w.propertyPath).sort()).toEqual([
            "layoutSizingVertical",
            "paddingTop",
        ]);
        expect(result.snapshot.root).toMatchObject({
            layoutSizingVertical: "fixed",
            autoLayout: {
                itemSpacing: -2,
                counterAxisSpacing: -5,
                paddingTop: 0,
            },
        });
    });
});

describe("desktop-grid", () => {
    test("group columns resolve world coordinates once", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/desktop-grid.json", "utf8"),
        );
        const result = await normalizeSource(source);
        if (!result.ok || result.empty || !("children" in result.snapshot.root))
            throw Error("Expected grid");
        const group = result.snapshot.root.children[0];
        if (!group || !("children" in group)) throw Error("Expected columns");
        expect(group.x).toBe(12);
        expect(group.children.map((n) => n.x)).toEqual(
            Array.from({ length: 12 }, (_, i) => i * 30),
        );
        expect(
            group.children.every((n) => n.width === 24 && n.height === 120),
        ).toBe(true);
        expect(result.warnings).toEqual([]);
        expect(source.root.children[0].children[0].properties.x).toBe(12);
    });
});

describe("geometry-roundoff", () => {
    async function fixture(): Promise<SourceCapture> {
        return JSON.parse(
            await readFile("fixtures/source/geometry-roundoff.json", "utf8"),
        );
    }

    test("near-zero negative width is repaired without mutating source", async () => {
        const source = await fixture();
        const before = JSON.stringify(source);
        const normalized = await normalizeSource(source);
        expect(normalized).toMatchObject({
            ok: true,
            snapshot: { root: { width: 0, height: 28 } },
            warnings: [
                {
                    code: "GEOMETRY_ROUNDOFF_APPROXIMATED",
                    propertyPath: "width",
                    originalValue: "-0.000001",
                    category: "geometry",
                },
            ],
        });
        expect(JSON.stringify(source)).toBe(before);
        expect(await normalizeSource(source)).toEqual(normalized);
        if (!normalized.ok || normalized.empty) throw Error("Missing snapshot");
        expect(convertSnapshot(normalized.snapshot).ok).toBe(true);
    });

    test.each(["width", "height"])(
        "%s roundoff tolerance is bounded and does not erase positive extents",
        async (dimension) => {
            for (const value of [-0.00001, 0, 0.000001]) {
                const source = await fixture();
                source.root.properties.width = 0;
                source.root.properties[dimension] = value;
                const result = await normalizeSource(source);
                expect(result.ok).toBe(true);
                if (!result.ok || result.empty) throw Error("Missing snapshot");
                expect(
                    result.snapshot.root[dimension as "width" | "height"],
                ).toBe(Math.max(0, value));
            }
            for (const value of [
                -0.00001001,
                -1,
                { $source: "NaN" },
                { $source: "Infinity" },
            ]) {
                const source = await fixture();
                source.root.properties.width = 0;
                source.root.properties[dimension] = value;
                expect(await normalizeSource(source)).toMatchObject({
                    ok: false,
                    diagnostics: [
                        expect.objectContaining({ code: "INVALID_GEOMETRY" }),
                    ],
                });
            }
        },
    );
});

describe("group-positioning", () => {
    async function fixture() {
        return JSON.parse(
            await readFile("fixtures/source/group-positioning.json", "utf8"),
        );
    }

    test("group children use the world origin once without mutating the source", async () => {
        const source = await fixture();
        const before = structuredClone(source);
        expect(await normalizeSource(source)).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    children: [
                        {
                            x: 13,
                            y: 83,
                            children: [{ id: "mark", x: 20, y: 0 }],
                        },
                    ],
                },
            },
        });
        expect(source).toEqual(before);
        source.root = source.root.children[0];
        source.root.properties.x = 0;
        source.root.properties.y = 0;
        expect(await normalizeSource(source)).toMatchObject({
            ok: true,
            snapshot: { root: { x: 0, y: 0, children: [{ x: 20, y: 0 }] } },
        });
    });

    test("rotated groups invert their transform while frames retain local positions", async () => {
        const source = await fixture();
        const group = source.root.children[0];
        group.properties.absoluteTransform = [
            [0, -1, 100],
            [1, 0, 200],
        ];
        group.children[0].properties.absoluteTransform = [
            [0, -1, 80],
            [1, 0, 210],
        ];
        expect(await normalizeSource(source)).toMatchObject({
            ok: true,
            snapshot: {
                root: { children: [{ children: [{ x: 10, y: 20 }] }] },
            },
        });
        group.type = "FRAME";
        expect(await normalizeSource(source)).toMatchObject({
            ok: true,
            snapshot: {
                root: { children: [{ children: [{ x: 33, y: 83 }] }] },
            },
        });
    });
});

describe("layout-depth", () => {
    test("deep layouts retain descendants and resume native layout after recovery boundaries", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/negative-gap.json", "utf8"),
        );
        let parent = source.root;
        for (let i = 0; i < 40; i++) {
            const child = structuredClone(source.root);
            child.id = `depth:${i}`;
            child.children = [];
            parent.children = [child];
            parent = child;
        }
        const before = JSON.stringify(source);
        const result = await normalizeSource(source);
        if (!result.ok || result.empty)
            throw Error("Expected recovered layout");
        const boundaries = result.warnings.filter(
            (w) => w.code === "LAYOUT_DEPTH_GEOMETRY_APPROXIMATED",
        );
        expect(boundaries.length).toBeGreaterThan(0);
        const nodes: SnapshotNode[] = [];
        function visit(node: SnapshotNode) {
            nodes.push(node);
            if ("children" in node) node.children.forEach(visit);
        }
        visit(result.snapshot.root);
        expect(nodes).toHaveLength(41);
        for (const warning of boundaries)
            expect(nodes.find((n) => n.id === warning.nodeId)).toMatchObject({
                autoLayout: null,
            });
        expect(
            nodes
                .slice(20)
                .some((n) => "autoLayout" in n && n.autoLayout !== null),
        ).toBe(true);
        expect(JSON.stringify(source)).toBe(before);
    });
});

describe("reverse-paint-order", () => {
    test("reverses flex painting while retaining the original child layout order", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/reverse-paint-order.json", "utf8"),
        );
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Expected reverse-order snapshot");
        expect(normalized.snapshot.root).toMatchObject({
            autoLayout: { reversePaintOrder: true },
            children: [{ id: "source:1" }, { id: "blue" }],
        });
        expect(
            normalized.warnings.map((warning) => warning.code),
        ).not.toContain("REVERSE_Z_ORDER_APPROXIMATED");
        const converted = convertSnapshot(normalized.snapshot);
        if (!converted.ok) throw Error("Expected reverse-order Slint");
        expect(converted.source.indexOf("background: #0000FF")).toBeLessThan(
            converted.source.indexOf("background: #FF0000"),
        );
        expect(converted.source).toContain("layout-order: 1;");
        expect(converted.source).toContain("layout-order: 0;");

        const mixed = structuredClone(source);
        mixed.root.children[1].properties.layoutPositioning = "ABSOLUTE";
        const fallback = await normalizeSource(mixed);
        if (!fallback.ok || fallback.empty)
            throw Error("Expected geometry fallback");
        expect(fallback.snapshot.root).toMatchObject({
            autoLayout: null,
            layoutFallback: "freeform",
            children: [{ id: "blue" }, { id: "source:1" }],
        });
        expect(fallback.warnings.map((warning) => warning.code)).toContain(
            "REVERSE_Z_ORDER_GEOMETRY_APPROXIMATED",
        );
    });
});

describe("scaled-instance", () => {
    test("uses resolved descendant geometry for scaled instances without scaling twice", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/scaled-instance.json", "utf8"),
        );
        const scaled = await normalizeSource(source);
        if (!scaled.ok || scaled.empty) throw Error("Expected scaled instance");
        const unscaledSource = structuredClone(source);
        unscaledSource.root.properties.scaleFactor = 1;
        const unscaled = await normalizeSource(unscaledSource);
        if (!unscaled.ok || unscaled.empty)
            throw Error("Expected control instance");

        expect(source.root.properties.scaleFactor).toBeCloseTo(2 / 3);
        expect(source.root.children[1].properties.fontSize).toBe(10);
        expect(scaled.snapshot).toEqual(unscaled.snapshot);
        expect(scaled.snapshot.root).toMatchObject({
            kind: "instance",
            width: 32,
            height: 32,
            children: [
                { width: 32, height: 32 },
                { kind: "text", width: 32, height: 32, fontSize: 10 },
            ],
        });
        expect(scaled.warnings.map((warning) => warning.code)).not.toContain(
            "INSTANCE_SCALE_APPROXIMATED",
        );
        const scaledSlint = convertSnapshot(scaled.snapshot);
        const unscaledSlint = convertSnapshot(unscaled.snapshot);
        expect(scaledSlint).toEqual(unscaledSlint);
    });
});
