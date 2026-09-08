// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { normalizeSource } from "../src/plugin/normalize";
import type { SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";

describe("recovery", () => {
    test("bad appearance entries and failed child exports preserve siblings", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/basic.json", "utf8"),
        );
        const rectangle = structuredClone(source.root);
        rectangle.id = "good";
        const broken = structuredClone(rectangle);
        broken.id = "broken";
        broken.type = "VECTOR";
        broken.exports = {
            svg: { error: "export unavailable" },
            png: { error: "export unavailable" },
        };
        source.root.type = "FRAME";
        source.root.children = [broken, rectangle];
        Object.assign(source.root.properties, {
            layoutMode: "NONE",
            cornerRadius: -1,
            fills: [
                { type: "GRADIENT_RADIAL" },
                { type: "SOLID", color: { r: 1, g: 0, b: 0 }, opacity: 1 },
            ],
            effects: [{ type: "DROP_SHADOW", radius: -1 }],
        });
        const result = await normalizeSource(source);
        if (!result.ok || result.empty)
            throw new Error("Expected usable preview");
        expect(result.snapshot.root).toMatchObject({
            cornerRadii: [0, 0, 0, 0],
            children: [
                { id: "broken", kind: "group" },
                { id: "good", kind: "rectangle" },
            ],
        });
        expect(result.warnings.length).toBeGreaterThanOrEqual(4);
        expect(convertSnapshot(result.snapshot).ok).toBe(true);
    });

    test("unusable geometry is omitted locally but fails an entirely unusable selection", async () => {
        const source: SourceCapture = JSON.parse(
            await readFile("fixtures/source/basic.json", "utf8"),
        );
        source.root.properties.width = { $source: "NaN" };
        expect(await normalizeSource(source)).toMatchObject({ ok: false });
        const bad = structuredClone(source.root);
        bad.id = "bad";
        source.root.properties.width = 100;
        source.root.type = "GROUP";
        source.root.children = [bad];
        const result = await normalizeSource(source);
        expect(result).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ code: "NODE_OMITTED" }),
                expect.objectContaining({ code: "UNRENDERABLE_SELECTION" }),
            ],
        });
    });
});

describe("real-source", () => {
    test("saved simultaneous failures preserve siblings and unsupported containers preserve native descendants", async () => {
        const failed: SourceCapture = JSON.parse(
            await readFile("fixtures/source/multiple-failures.json", "utf8"),
        );
        const result = await normalizeSource(failed);
        if (!result.ok || result.empty) throw Error("Expected partial preview");
        expect(result.warnings.map((w) => w.code)).toEqual(
            expect.arrayContaining([
                "APPEARANCE_APPROXIMATED",
                "NODE_PLACEHOLDER",
                "EFFECT_OMITTED",
            ]),
        );
        expect(result.snapshot.root).toMatchObject({
            children: [
                { id: "broken", kind: "group" },
                { id: "good", kind: "rectangle" },
            ],
        });
        expect(convertSnapshot(result.snapshot).ok).toBe(true);
        const nested: SourceCapture = JSON.parse(
            await readFile("fixtures/source/nested-container.json", "utf8"),
        );
        const retained = await normalizeSource(nested);
        if (!retained.ok || retained.empty)
            throw Error("Expected retained descendants");
        expect(retained.snapshot.root).toMatchObject({
            kind: "container",
            children: [
                {
                    id: "native-child",
                    autoLayout: { direction: "vertical", itemSpacing: 4 },
                    children: [{ id: "leaf" }],
                },
            ],
        });
        expect(retained.warnings.map((w) => w.code)).toContain(
            "CONTAINER_TYPE_APPROXIMATED",
        );
    });
});
