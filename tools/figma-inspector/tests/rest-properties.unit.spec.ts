// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { restProperty, REST_UNAVAILABLE } from "../src/plugin/rest-properties";

test("bulk metadata reuses matching fields and documented defaults", () => {
    expect(restProperty({ width: 24 }, "width")).toBe(24);
    expect(restProperty({}, "opacity")).toBe(1);
    expect(restProperty({}, "visible")).toBe(true);
    expect(restProperty({}, "effects")).toEqual([]);
    expect(restProperty({}, "unknown")).toBe(REST_UNAVAILABLE);
});
test("schema differences and context-sensitive paints retain API reads", () => {
    for (const key of [
        "rotation",
        "fontName",
        "boundVariables",
        "resolvedVariableModes",
        "componentPropertyReferences",
        "strokeWeight",
        "layoutAlign",
    ])
        expect(restProperty({ [key]: 1 }, key)).toBe(REST_UNAVAILABLE);
    for (const paint of [
        { type: "IMAGE", imageRef: "image" },
        { type: "GRADIENT_LINEAR", gradientHandlePositions: [] },
        { type: "SOLID", color: { a: 0.5 } },
        { type: "SOLID", boundVariables: { color: { id: "theme" } } },
    ])
        expect(restProperty({ fills: [paint] }, "fills")).toBe(
            REST_UNAVAILABLE,
        );
    const fills = [
        { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 }, opacity: 1 },
    ];
    expect(restProperty({ fills }, "fills")).toBe(fills);
    expect(
        restProperty({ effects: [{ type: "DROP_SHADOW" }] }, "effects"),
    ).toBe(REST_UNAVAILABLE);
});

test("vector corner rounding retains direct reads when bulk JSON omits it", () => {
    for (const key of ["cornerRadius", "topLeftRadius", "bottomRightRadius"])
        expect(restProperty({ type: "VECTOR" }, key)).toBe(REST_UNAVAILABLE);
    expect(restProperty({ type: "RECTANGLE" }, "cornerRadius")).toBe(0);
});

test("REST paint opacity defaults to one without changing the original document", () => {
    const paint = { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 } };
    expect(restProperty({ fills: [paint] }, "fills")).toEqual([
        { ...paint, opacity: 1 },
    ]);
    expect(paint).not.toHaveProperty("opacity");
    for (const opacity of [null, "1", -1, 2, Number.NaN])
        expect(
            restProperty({ strokes: [{ ...paint, opacity }] }, "strokes"),
        ).toBe(REST_UNAVAILABLE);
});

test("text fills retain API reads because runs can override them", () => {
    const fills = [
        { type: "SOLID", color: { r: 0, g: 0, b: 0, a: 1 }, opacity: 1 },
    ];
    expect(restProperty({ type: "TEXT", fills }, "fills")).toBe(
        REST_UNAVAILABLE,
    );
    expect(restProperty({ type: "RECTANGLE", fills }, "fills")).toBe(fills);
});

test("real Figma REST defaults and solid paints match paired live reads", async () => {
    const { rest, live } = JSON.parse(
        await readFile("fixtures/authored/rest-live.json", "utf8"),
    );
    let defaults = 0;
    let reused = 0;
    // Compare paint semantics: REST includes color alpha and omits default visibility;
    // the plugin API has RGB colors and an empty variable-binding object instead.
    const paints = (value: unknown) =>
        (value as SolidPaint[]).map((paint) => ({
            type: paint.type,
            color: { r: paint.color.r, g: paint.color.g, b: paint.color.b },
            opacity: paint.opacity,
            visible: paint.visible !== false,
            blendMode: paint.blendMode,
        }));
    for (const node of [rest.document, ...rest.document.children]) {
        for (const [key, expected] of Object.entries(live[node.id])) {
            const actual = restProperty(node, key);
            if (actual === REST_UNAVAILABLE) continue;
            reused++;
            if (!(key in node)) defaults++;
            expect(
                key === "fills" || key === "strokes" ? paints(actual) : actual,
                `${node.name}.${key}`,
            ).toEqual(
                key === "fills" || key === "strokes"
                    ? paints(expected)
                    : expected,
            );
        }
    }
    expect(reused).toBeGreaterThan(70);
    expect(defaults).toBeGreaterThan(40);
    expect(rest.document.children[0].fills[0]).not.toHaveProperty("opacity");
    expect(rest.document.children[1].fills[0].opacity).toBeCloseTo(0.4);
    expect(rest.document.children[1].opacity).toBeCloseTo(0.7);
});
