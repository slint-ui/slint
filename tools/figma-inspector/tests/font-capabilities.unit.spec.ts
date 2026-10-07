// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:words wght opsz

import { expect, test } from "vitest";
import { embeddedFontSupports } from "../src/plugin/font-capabilities";
import { textRequiresRasterPreview } from "../src/plugin/normalization-text";
import type { MaterializedNode } from "../src/plugin/normalization-context";

test.each([
    ["Inter", "Regular", 400, undefined, "Hello", true],
    ["Inter", "Bold", 700, undefined, "Hello", true],
    ["Inter", "Italic", 400, undefined, "Hello", false],
    ["Inter", "Regular", 400, { wght: 650 }, "Hello", true],
    ["Inter", "Regular", 400, { wght: 950 }, "Hello", false],
    ["Inter", "Regular", 400, { wght: 650.5 }, "Hello", false],
    ["Inter", "Regular", 400, { ital: 1 }, "Hello", false],
    ["Inter", "Regular", 400, { opsz: 14 }, "Hello", false],
    ["Inter", "Regular", 400, undefined, "Custom\uE000 glyph", false],
    ["Inter", "Regular", 400, undefined, "\u{F0000}", false],
    ["Inter", "Regular", 400, undefined, "\u4E00", false],
    ["Inter", "Regular", 400, undefined, "Two\nlines", true],
    ["Example Font", "Regular", 400, undefined, "Hello", false],
] as const)(
    "embedded font capabilities: %s/%s/%s/%j/%s",
    (family, style, weight, variations, characters, supported) => {
        expect(
            embeddedFontSupports(
                { family, style },
                weight,
                variations,
                characters,
            ),
        ).toBe(supported);
    },
);

test("mixed weights use complete segment capabilities rather than an unresolved base weight", () => {
    const font = { family: "Inter", style: "Regular" };
    const node = {
        characters: "ab",
        fontName: font,
        fontWeight: Symbol("mixed"),
        sourceSegments: {
            segments: [
                { characters: "a", fontName: font, fontWeight: 400 },
                { characters: "b", fontName: font, fontWeight: 700 },
            ],
        },
    } as unknown as MaterializedNode;
    expect(textRequiresRasterPreview(node, Symbol())).toBe(false);
    const incomplete = {
        ...node,
        sourceSegments: {
            segments: [{ characters: "a", fontName: font, fontWeight: 400 }],
        },
    } as unknown as MaterializedNode;
    expect(textRequiresRasterPreview(incomplete, Symbol())).toBe(true);
});
