// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { normalizeSource } from "../src/plugin/normalize";
import { validateSnapshot } from "../src/plugin/snapshot";
import { convertSnapshot } from "../src/preview/converter";
import { scalePreview } from "../src/preview/scale-preview";
import { repeatedPreview } from "./repeated-preview";
import { convertCapture } from "../src/preview/convert-capture";
import { previewAssetSource } from "./preview-asset-source";

async function seed() {
    return JSON.parse(
        await readFile("fixtures/source/inner-shadow.json", "utf8"),
    );
}

async function convert(source: Awaited<ReturnType<typeof seed>>) {
    const normalized = await normalizeSource(source, "export");
    if (!normalized.ok || normalized.empty)
        throw Error(JSON.stringify(normalized));
    const converted = convertSnapshot(normalized.snapshot);
    if (!converted.ok) throw Error(JSON.stringify(converted));
    expect(validateSnapshot(normalized.snapshot).ok).toBe(true);
    return { normalized, converted };
}

test("native inset shadows and normal backdrop blur retain other valid effects", async () => {
    const source = await seed();
    source.root.properties.effects.push(
        { type: "BACKGROUND_BLUR", visible: true, radius: 12 },
        {
            type: "BACKGROUND_BLUR",
            visible: true,
            radius: 30,
            blurType: "PROGRESSIVE",
        },
        { type: "LAYER_BLUR", visible: true, radius: 5 },
        {
            type: "BACKGROUND_BLUR",
            visible: true,
            radius: 10,
            blurType: "NORMAL",
        },
        { type: "INNER_SHADOW", visible: true, radius: -1 },
    );
    const { normalized, converted } = await convert(source);
    expect(converted.source).toContain("inner-shadow-spread: 2px;");
    expect(converted.source).toContain("backdrop-blur: 6px;");
    expect(converted.source).not.toContain("@linear-gradient(");
    expect(normalized.warnings.map((warning) => warning.code)).toContain(
        "UNSUPPORTED_EFFECT_IGNORED",
    );
    const scaled = scalePreview(normalized.snapshot, 0.5);
    expect(scaled.root).toMatchObject({
        backdropBlur: 3,
        shadows: [{ blur: 6, spread: 1 }],
    });
    expect(normalized.snapshot.root).toMatchObject({
        backdropBlur: 6,
        shadows: [{ blur: 12, spread: 2 }],
    });
});

test("normal drop shadows preserve show-behind and diagnose knockout limitations", async () => {
    const source = await seed();
    const effect = source.root.properties.effects[0];
    effect.type = "DROP_SHADOW";
    effect.showShadowBehindNode = false;
    source.root.properties.fills[0].opacity = 0.5;
    const result = await convert(source);
    expect(result.normalized.snapshot.root).toMatchObject({
        shadows: [{ showBehind: false }],
    });
    expect(result.converted.warnings.map((warning) => warning.code)).toContain(
        "SHADOW_TRANSPARENCY_APPROXIMATED",
    );
    effect.showShadowBehindNode = true;
    expect(
        (await convert(source)).converted.warnings.map(
            (warning) => warning.code,
        ),
    ).not.toContain("SHADOW_TRANSPARENCY_APPROXIMATED");
});

test("offset linear gradients retain fractional stops and resize with the element", async () => {
    const source = await seed();
    source.root.properties.effects = [];
    source.root.properties.fills = [
        {
            type: "GRADIENT_LINEAR",
            opacity: 1,
            gradientTransform: [
                [1.25, 0.5, -0.25],
                [0, 1, 0],
            ],
            gradientStops: [
                { position: 0.125, color: { r: 1, g: 0, b: 0, a: 1 } },
                { position: 1, color: { r: 0, g: 0, b: 1, a: 0 } },
            ],
        },
    ];
    const { converted } = await convert(source);
    expect(converted.source).toContain(
        "Math.atan2(0.5 * self.width / 1px, 1.25 * self.height / 1px)",
    );
    expect(converted.source).toContain("21.4285714286%");
    expect(converted.source).toContain("71.4285714286%");
});

test("radial and circular angular gradients are native while sheared radial fills retain SVG", async () => {
    const source = await seed();
    source.root.properties.width = source.root.properties.height = 60;
    source.root.properties.effects = [];
    const gradient = {
        type: "GRADIENT_RADIAL",
        opacity: 1,
        gradientTransform: [
            [1, 0, 0],
            [0, 2, -0.5],
        ],
        gradientStops: [
            { position: 0, color: { r: 1, g: 0, b: 0, a: 1 } },
            { position: 1, color: { r: 0, g: 0, b: 1, a: 1 } },
        ],
    };
    source.root.properties.fills = [gradient];
    expect((await convert(source)).converted.source).toContain(
        "@radial-gradient(ellipse self.width * 0.5 self.height * 0.25",
    );
    gradient.gradientTransform = [
        [1, 0.25, 0],
        [0, 1, 0],
    ];
    expect((await convert(source)).converted.source).toContain("@image-url(");
    source.root.properties.strokes = [gradient];
    source.root.properties.strokeWeight = 2;
    expect(
        (await convert(source)).normalized.warnings.map(
            (warning) => warning.code,
        ),
    ).toContain("UNSUPPORTED_STROKE_PAINT");
    source.root.properties.strokes = [];
    gradient.type = "GRADIENT_ANGULAR";
    gradient.gradientTransform = [
        [1, 0, 0],
        [0, 1, 0],
    ];
    expect((await convert(source)).converted.source).toContain(
        "@conic-gradient(from 90deg",
    );
    gradient.gradientTransform = [
        [1, 0, 0],
        [0, 2, 0],
    ];
    expect(
        (await convert(source)).normalized.warnings.map(
            (warning) => warning.code,
        ),
    ).toContain("UNSUPPORTED_PAINT_IGNORED");
});

test.each(["INSIDE", "CENTER", "OUTSIDE"])(
    "uniform %s dashes use a stroked path and unequal sides keep the fallback",
    async (align) => {
        const source = await seed();
        const p = source.root.properties;
        p.effects = [];
        p.strokes = p.fills;
        p.strokeWeight = 2;
        p.strokeAlign = align;
        p.strokeCap = "ROUND";
        p.strokeJoin = "BEVEL";
        p.dashPattern = [6, 4];
        const { normalized, converted } = await convert(source);
        expect(converted.source).toContain("stroke-dash-array:");
        expect(converted.source).toContain("stroke-dash-offset:");
        expect(converted.source).toContain("stroke-line-cap: round;");
        expect(converted.source).toContain("stroke-line-join: bevel;");
        expect(converted.source).not.toContain("border-width:");
        expect(
            normalized.warnings.map((warning) => warning.code),
        ).not.toContain("DASHED_STROKE_APPROXIMATED");
        p.strokeWeight = { $source: "mixed" };
        [
            p.strokeTopWeight,
            p.strokeRightWeight,
            p.strokeBottomWeight,
            p.strokeLeftWeight,
        ] = [1, 2, 3, 4];
        expect(
            (await convert(source)).normalized.warnings.map(
                (warning) => warning.code,
            ),
        ).toContain("DASHED_STROKE_APPROXIMATED");
    },
);

test("plain text outlines survive export and preview scaling", async () => {
    const source = await seed();
    source.root.type = "TEXT";
    Object.assign(source.root.properties, {
        effects: [],
        characters: "Outline",
        fontName: { family: "Inter", style: "Regular" },
        fontSize: 16,
        fontWeight: 400,
        textAlignHorizontal: "LEFT",
        textAlignVertical: "TOP",
        textAutoResize: "NONE",
        lineHeight: { unit: "AUTO" },
        letterSpacing: { unit: "PIXELS", value: 0 },
        strokeAlign: "OUTSIDE",
        strokeWeight: 2,
        strokes: source.root.properties.fills,
    });
    const { normalized, converted } = await convert(source);
    expect(converted.source).toContain("stroke-width: 2px;");
    expect(converted.source).toContain("stroke-style: outside;");
    expect(scalePreview(normalized.snapshot, 0.5).root).toMatchObject({
        outline: { width: 1 },
    });
    source.root.properties.strokeAlign = "INSIDE";
    expect(
        (await convert(source)).normalized.warnings.map(
            (warning) => warning.code,
        ),
    ).toContain("TEXT_OUTLINE_APPROXIMATED");
});

test("inner shadows stay below inside strokes and retain bordered drop-shadow casters", async () => {
    const source = await seed();
    Object.assign(source.root.properties, {
        strokes: [{ type: "SOLID", opacity: 0.5, color: { r: 1, g: 0, b: 0 } }],
        strokeWeight: 3,
        strokeAlign: "INSIDE",
        clipsContent: true,
    });
    const inner = (await convert(source)).converted.source;
    expect(inner.indexOf("inner-shadow-blur:")).toBeLessThan(
        inner.indexOf("border-width:"),
    );
    expect(inner.match(/border-width: 3px;/gu)).toHaveLength(1);
    source.root.properties.effects.push({
        ...source.root.properties.effects[0],
        type: "DROP_SHADOW",
        showShadowBehindNode: true,
    });
    const result = (await convert(source)).converted;
    const outer = result.source;
    expect(outer.indexOf("drop-shadow-blur:")).toBeLessThan(
        outer.indexOf("inner-shadow-blur:"),
    );
    expect(outer.lastIndexOf("border-width:")).toBeGreaterThan(
        outer.indexOf("inner-shadow-blur:"),
    );
    expect(outer.match(/background:/gu)).toHaveLength(2);
    expect(outer.match(/border-width: 3px;/gu)).toHaveLength(1);
    expect(result.warnings).toEqual([]);
    source.root.properties.fills[0].opacity = 0.5;
    const translucent = (await convert(source)).converted;
    expect(translucent.source.match(/background:/gu)).toHaveLength(4);
    expect(translucent.warnings.map((warning) => warning.code)).toContain(
        "SHADOW_PAINT_APPROXIMATED",
    );
});

test("repeated component templates retain different native shadow and blur values", async () => {
    const source = repeatedPreview(
        JSON.parse(
            await readFile("fixtures/source/conditional-root.json", "utf8"),
        ),
        8,
    );
    for (const [index, node] of (source.root.children ?? []).entries()) {
        node.properties.effects = [
            {
                type: "INNER_SHADOW",
                visible: true,
                radius: 4 + index,
                spread: index - 2,
                offset: { x: index, y: -index },
                color: { r: index / 7, g: 0, b: 1, a: 0.8 },
                blendMode: "NORMAL",
            },
            { type: "BACKGROUND_BLUR", radius: 2 * index, visible: true },
        ];
    }
    const { normalized } = await convert(source);
    const specialized = convertSnapshot(normalized.snapshot, {
        specialize: true,
    });
    if (!specialized.ok) throw Error(JSON.stringify(specialized));
    expect(specialized.source.match(/^component /gm)).toHaveLength(3);
    expect(specialized.source).toMatch(
        /inner-shadow-blur: root\.figma-value-\d+;/u,
    );
    expect(specialized.source).toMatch(
        /backdrop-blur: root\.figma-value-\d+;/u,
    );
    const scaled = scalePreview(normalized.snapshot, 0.5);
    if (!("children" in scaled.root)) throw Error("Missing children");
    expect(scaled.root.children[7]).toMatchObject({
        backdropBlur: 3.5,
        shadows: [{ blur: 5.5, spread: 2.5, offsetX: 3.5 }],
    });
});

test("native properties agree across preview, root-only snippets, and project exports", async () => {
    const captureJson = await readFile(
        "fixtures/source/native-paints-effects.json",
        "utf8",
    );
    const output = await convertCapture({
        type: "preview-capture",
        revision: 9,
        captureJson,
    });
    if (output.type !== "preview-source" || !output.exportPackage)
        throw Error(JSON.stringify(output));
    const preview =
        typeof output.source === "string"
            ? output.source
            : previewAssetSource(output.source);
    const normalized = await normalizeSource(JSON.parse(captureJson), "export");
    if (
        !normalized.ok ||
        normalized.empty ||
        !("children" in normalized.snapshot.root)
    )
        throw Error(JSON.stringify(normalized));
    const feature =
        /^\s*(?:inner-shadow-[\w-]+:|drop-shadow-[\w-]+:|backdrop-blur:|stroke-dash-[\w-]+:|stroke-style:|stroke-width:|stroke:|(?:background|border-color):.*@(?:linear|radial|conic)-gradient)/u;
    let checked = 0;
    for (const child of normalized.snapshot.root.children) {
        const snippet = convertSnapshot(
            { ...normalized.snapshot, root: child },
            { scope: "root-only" },
        );
        if (!snippet.ok) throw Error(JSON.stringify(snippet));
        for (const line of snippet.source
            .split("\n")
            .filter((line) => feature.test(line))) {
            expect(output.exportPackage.source).toContain(line.trim());
            if (child.kind !== "text") expect(preview).toContain(line.trim());
            checked++;
        }
    }
    expect(checked).toBeGreaterThan(40);
    expect(output.exportPackage.source).toContain("stroke-style: center;");
});

test("invalid effects preserve clipped, translucent parents and their valid children", async () => {
    const source = await seed();
    const child = structuredClone(source.root);
    child.id = "valid-child";
    child.properties.effects = [];
    source.root.type = "FRAME";
    source.root.children = [child];
    Object.assign(source.root.properties, { opacity: 0.6, clipsContent: true });
    source.root.properties.effects.push({
        type: "INNER_SHADOW",
        radius: -1,
        visible: true,
    });
    const { normalized, converted } = await convert(source);
    expect(normalized.snapshot.root).toMatchObject({
        opacity: 0.6,
        clipsContent: true,
        children: [{ id: "valid-child" }],
    });
    expect(converted.source).toContain("inner-shadow-blur: 12px;");
    expect(converted.source).toContain("clip: true;");
    expect(converted.source).toContain("opacity: 0.6;");
    expect(normalized.warnings.map((warning) => warning.code)).toContain(
        "EFFECT_OMITTED",
    );
});

test("schema 9 rejects malformed new properties without accepting schema 8", async () => {
    const { normalized } = await convert(await seed());
    if (!("shadows" in normalized.snapshot.root))
        throw Error("Missing shadows");
    for (const changed of [
        { backdropBlur: -1 },
        { backdropBlur: Infinity },
        {
            shadows: [
                { ...normalized.snapshot.root.shadows[0], showBehind: "yes" },
            ],
        },
        {
            strokes: [
                {
                    paint: {
                        kind: "solid",
                        opacity: 1,
                        color: { r: 0, g: 0, b: 0, a: 1 },
                    },
                    strokeTopWeight: 1,
                    strokeRightWeight: 1,
                    strokeBottomWeight: 1,
                    strokeLeftWeight: 1,
                    dashPattern: [],
                    lineCap: "invalid",
                },
            ],
        },
    ]) {
        expect(
            validateSnapshot({
                ...normalized.snapshot,
                root: { ...normalized.snapshot.root, ...changed },
            }).ok,
        ).toBe(false);
    }
    expect(
        validateSnapshot({ ...normalized.snapshot, schemaVersion: 8 }).ok,
    ).toBe(false);
});
