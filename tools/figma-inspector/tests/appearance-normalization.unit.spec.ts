// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import type { SourceCapture } from "../src/plugin/source";

describe("asymmetric-rounded-stroke", () => {
    test("joins asymmetric rounded strokes as one border-ring path", async () => {
        const source = JSON.parse(
            await readFile(
                "fixtures/source/asymmetric-rounded-stroke.json",
                "utf8",
            ),
        );
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Expected stroke snapshot");
        const converted = convertSnapshot(normalized.snapshot);
        if (!converted.ok) throw Error("Expected stroke Slint");
        expect(converted.source).toContain("Path {");
        expect(converted.source).toContain('commands: "M 12 0');
        expect(converted.source).toContain("fill: #FF0000;");
        expect(converted.source).not.toContain("parent.width - 2px");
        expect(converted.warnings.map((warning) => warning.code)).not.toEqual(
            expect.arrayContaining([
                "ASYMMETRIC_STROKE_CORNERS_APPROXIMATED",
                "ASYMMETRIC_GRADIENT_STROKE_APPROXIMATED",
            ]),
        );
    });
});

describe("stroke-alignment", () => {
    test("preserves outside and centered stroke geometry around the layout box", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/stroke-alignment.json", "utf8"),
        );
        const outside = await normalizeSource(source);
        if (!outside.ok || outside.empty)
            throw Error("Expected outside stroke");
        expect(outside.snapshot.root).toMatchObject({
            width: 100,
            height: 60,
            strokes: [{ align: "outside" }],
        });
        expect(outside.warnings.map((warning) => warning.code)).not.toContain(
            "STROKE_ALIGNMENT_IGNORED",
        );
        const outsideSlint = convertSnapshot(outside.snapshot);
        if (!outsideSlint.ok) throw Error("Expected outside Slint");
        expect(outsideSlint).toMatchObject({ width: 120, height: 80 });
        expect(outsideSlint.source).toContain("x: -10px;");
        expect(outsideSlint.source).toContain("width: 120px;");
        expect(outsideSlint.source).toMatch(/Rectangle \{\n\s+clip: true;/u);
        expect(outsideSlint.source).not.toMatch(
            /Rectangle \{\n\s+x: 0px;\n\s+y: 0px;\n\s+width: 100%;\n\s+height: 100%;\n\s+clip: true;/u,
        );

        source.root.properties.strokeAlign = "CENTER";
        const centered = await normalizeSource(source);
        if (!centered.ok || centered.empty)
            throw Error("Expected centered stroke");
        expect(centered.snapshot.root).toMatchObject({
            strokes: [{ align: "center" }],
        });
        const centeredSlint = convertSnapshot(centered.snapshot);
        if (!centeredSlint.ok) throw Error("Expected centered Slint");
        expect(centeredSlint).toMatchObject({ width: 110, height: 70 });
        expect(centeredSlint.source).toContain("x: -5px;");
        expect(centeredSlint.source).toContain("width: 110px;");
    });
});

describe("inner-shadow", () => {
    test("inset and drop shadows coexist while invalid inset entries are isolated", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/inner-shadow.json", "utf8"),
        );
        const effect = source.root.properties.effects[0];
        source.root.properties.effects.push(
            { ...effect, type: "DROP_SHADOW" },
            { ...effect, radius: -1 },
        );
        const result = await normalizeSource(source);
        if (!result.ok || result.empty) throw Error("Expected shadow preview");
        expect(result.snapshot.root).toMatchObject({
            shadows: [{ kind: "inner" }, { blur: 12 }],
        });
        expect(result.warnings.map((w) => w.code)).toEqual(
            expect.arrayContaining([
                "INNER_SHADOW_APPROXIMATED",
                "EFFECT_OMITTED",
            ]),
        );
        const converted = convertSnapshot(result.snapshot);
        if (!converted.ok) throw Error("Expected Slint");
        expect(converted.source).toContain("drop-shadow-color:");
        expect(converted.source).toContain("@linear-gradient(90deg");
        expect(converted.source).not.toMatch(
            /Rectangle \{\n\s+x: 0px;\n\s+y: 0px;\n\s+clip: true;/u,
        );
        expect(converted.source).not.toContain("@image-url");
    });
});

describe("baseline-alignment", () => {
    test("baseline rows preserve the captured offset of smaller percentage text", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/baseline-alignment.json", "utf8"),
        );
        const before = JSON.stringify(source);
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Expected snapshot");
        expect(normalized.snapshot.root).toMatchObject({
            autoLayout: null,
            layoutFallback: "freeform",
            children: [
                { x: 0, y: 0, width: 176, height: 160 },
                { x: 184, y: 104, width: 48, height: 56 },
            ],
        });
        const converted = convertSnapshot(normalized.snapshot);
        if (!converted.ok) throw Error("Expected Slint");
        expect(converted.source).toContain("y: 104px;");
        expect(converted.source).not.toContain("FlexboxLayout {");
        expect(JSON.stringify(source)).toBe(before);
        expect(await normalizeSource(JSON.parse(before))).toEqual(normalized);

        source.root.properties.counterAxisAlignItems = "MAX";
        const supported = await normalizeSource(source);
        if (!supported.ok || supported.empty) throw Error("Expected snapshot");
        expect(supported.snapshot.root).toMatchObject({
            layoutFallback: "flex",
            autoLayout: { counterAlignment: "end" },
        });
    });
});
