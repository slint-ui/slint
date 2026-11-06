// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { solidVectorSvg, unpaintedVectorSvg } from "../src/plugin/vector-svg";
import { validSvgDocument } from "../src/images";

const properties = {
    width: 24,
    height: 24,
    opacity: 0.5,
    effects: [],
    strokes: [],
    cornerRadius: 0,
    fills: [{ type: "SOLID", color: { r: 1, g: 0, b: 0.5 }, opacity: 0.8 }],
};
const paths = [
    {
        windingRule: "EVENODD",
        data: "M 0 0 L 24 0 L 24 24 Z M 4 4 L 8 4 L 8 8 Z",
    },
];
test("resolved solid paths preserve geometry, winding rules and group opacity", () => {
    const svg = solidVectorSvg(properties, paths);
    expect(svg).toContain('viewBox="0 0 24 24"');
    expect(svg).toContain(
        '<g opacity="0.5" fill="#ff0080" fill-opacity="0.8">',
    );
    expect(svg).toContain('fill-rule="evenodd"');
    expect(validSvgDocument(svg!)).toBe(true);
});
test("complex or invalid vectors retain host export", () => {
    for (const changed of [
        { strokes: [{}] },
        { effects: [{}] },
        { cornerRadius: 2 },
        { width: 0 },
        { opacity: NaN },
        { fills: [{ type: "GRADIENT_LINEAR" }] },
        { fills: [...properties.fills, ...properties.fills] },
        { fills: [{ ...properties.fills[0], blendMode: "MULTIPLY" }] },
        { fills: [{ ...properties.fills[0], color: { r: -1, g: 0, b: 0 } }] },
    ])
        expect(
            solidVectorSvg({ ...properties, ...changed }, paths),
        ).toBeUndefined();
    expect(
        solidVectorSvg(properties, [
            { ...paths[0], data: 'M 0 0"/><script/>' },
        ]),
    ).toBeUndefined();
    expect(
        solidVectorSvg(properties, [{ ...paths[0], windingRule: "unknown" }]),
    ).toBeUndefined();
});

test("unpainted vector geometry has a transparent asset without host exports", () => {
    const properties = {
        width: 20,
        height: 20,
        fills: [],
        strokes: [],
        effects: [],
        fillStyleId: "",
    };
    const network = { regions: [{ fills: [], fillStyleId: "" }] };
    expect(unpaintedVectorSvg(properties, network)).toContain(
        'width="20" height="20"/>',
    );
    expect(
        unpaintedVectorSvg(properties, {
            regions: [{ fills: [{ type: "SOLID" }] }],
        }),
    ).toBeUndefined();
    expect(
        unpaintedVectorSvg(properties, {
            regions: [{ fills: [], fillStyleId: "paint-style" }],
        }),
    ).toBeUndefined();
    expect(
        unpaintedVectorSvg({ ...properties, isMask: true }, network),
    ).toBeUndefined();
    expect(
        unpaintedVectorSvg({ ...properties, effects: [{}] }, network),
    ).toBeUndefined();
    expect(
        unpaintedVectorSvg({ ...properties, strokes: [{}] }, network),
    ).toBeUndefined();
});
