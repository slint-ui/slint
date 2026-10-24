// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { convertSnapshotJson } from "../src/preview/converter";

test.each(["auto", "absolute"])(
    "converts 20,000 %s children in an auto-layout without overflowing the argument stack",
    async (layoutPositioning) => {
        const fixture = JSON.parse(
            await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
        );
        const child = fixture.root.children[0];
        fixture.root.children = Array.from({ length: 20_000 }, (_, index) => ({
            ...child,
            id: `large:${index}`,
            characters: `Child ${index}`,
            layoutPositioning,
        }));
        const result = convertSnapshotJson(JSON.stringify(fixture));
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source.match(/Text \{/gu)).toHaveLength(20_000);
        expect(result.source).toContain('text: "Child 0";');
        expect(result.source).toContain('text: "Child 19999";');
        expect(result.source.indexOf('text: "Child 0";')).toBeLessThan(
            result.source.indexOf('text: "Child 19999";'),
        );
    },
);

test("suppresses flex defaults, preserves no-wrap, and uses safe padding shorthand", async () => {
    const base = JSON.parse(
        await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
    ) as {
        root: {
            autoLayout: Record<string, unknown>;
            children: Array<Record<string, unknown>>;
        };
    };
    const defaults = convertSnapshotJson(JSON.stringify(base));
    expect(defaults.ok).toBe(true);
    if (!defaults.ok) return;
    expect(defaults.source).not.toContain("flex-direction: row;");
    expect(defaults.source).toContain("flex-wrap: no-wrap;");
    expect(defaults.source).not.toContain("alignment: start;");
    expect(defaults.source).toContain("cross-axis-alignment: center;");
    expect(defaults.source).not.toContain("spacing: 0px;");

    const promoted = {
        ...base,
        root: {
            ...base.root,
            autoLayout: {
                ...base.root.autoLayout,
                paddingLeft: 8,
                paddingRight: 8,
                paddingTop: 8,
                paddingBottom: 8,
                itemSpacing: 0,
                counterAxisSpacing: 0,
                primaryAlignment: "start",
                counterAlignment: "center",
                wrap: false,
            },
            children: [
                {
                    ...base.root.children[0],
                    layoutSizingHorizontal: "fill",
                },
                ...base.root.children.slice(1),
            ],
        },
    };
    const promotedResult = convertSnapshotJson(JSON.stringify(promoted));
    expect(promotedResult.ok).toBe(true);
    if (!promotedResult.ok) return;
    expect(promotedResult.source).toContain("padding: 8px;");
    expect(promotedResult.source).toContain("alignment: stretch;");
    expect(promotedResult.source).not.toContain("padding-left: 8px;");
    expect(promotedResult.source).not.toContain("spacing: 0px;");

    const nonDefaults = {
        ...base,
        root: {
            ...base.root,
            autoLayout: {
                ...base.root.autoLayout,
                direction: "vertical",
                paddingLeft: 4,
                paddingRight: 0,
                paddingTop: 6,
                paddingBottom: 4,
                itemSpacing: 5,
                counterAxisSpacing: 9,
                primaryAlignment: "end",
                counterAlignment: "center",
                wrap: true,
                counterAxisAlignContent: "center",
            },
        },
    };
    const nonDefaultResult = convertSnapshotJson(JSON.stringify(nonDefaults));
    expect(nonDefaultResult.ok).toBe(true);
    if (!nonDefaultResult.ok) return;
    expect(nonDefaultResult.source).toContain("flex-direction: column;");
    expect(nonDefaultResult.source).not.toContain("flex-wrap: wrap;");
    expect(nonDefaultResult.source).toContain("padding-left: 4px;");
    expect(nonDefaultResult.source).toContain("padding-top: 6px;");
    expect(nonDefaultResult.source).toContain("padding-bottom: 4px;");
    expect(nonDefaultResult.source).toContain("spacing: 5px;");
    expect(nonDefaultResult.source).toContain("spacing-horizontal: 9px;");
    expect(nonDefaultResult.source).toContain("alignment: end;");
    expect(nonDefaultResult.source).toContain("cross-axis-alignment: center;");
    expect(nonDefaultResult.source).toContain(
        "cross-axis-line-alignment: center;",
    );
});

test("removes only provably transparent root and image-fill wrappers", async () => {
    const button = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    ) as { root: Record<string, unknown> };
    const collapsedGroup = convertSnapshotJson(JSON.stringify(button));
    expect(collapsedGroup.ok).toBe(true);
    if (!collapsedGroup.ok) return;
    expect(collapsedGroup.source.match(/Rectangle \{/gu)?.length).toBe(1);

    const styledGroup = convertSnapshotJson(
        JSON.stringify({
            ...button,
            root: { ...button.root, opacity: 0.9 },
        }),
    );
    expect(styledGroup.ok).toBe(true);
    if (!styledGroup.ok) return;
    expect(styledGroup.source.match(/Rectangle \{/gu)?.length).toBe(2);

    const image = JSON.parse(
        await readFile("fixtures/image-fill.snapshot.json", "utf8"),
    ) as { root: Record<string, unknown> };
    const imageResult = convertSnapshotJson(JSON.stringify(image));
    expect(imageResult.ok).toBe(true);
    if (!imageResult.ok) return;
    expect(imageResult.source).toContain("Image {");
    expect(imageResult.source).not.toMatch(
        /Image \{\n\s+x: 0px;\n\s+y: 0px;\n\s+width: 100%;\n\s+height: 100%;/u,
    );
    expect(imageResult.source).toContain("image-fit: contain;");
    expect(imageResult.source).not.toContain("clip: true;");

    const scaledTile = convertSnapshotJson(
        JSON.stringify({
            ...image,
            root: {
                ...image.root,
                fills: [
                    {
                        ...(
                            image.root.fills as Array<Record<string, unknown>>
                        )[0],
                        scaleMode: "TILE",
                        tileScale: 2,
                    },
                ],
            },
        }),
    );
    expect(scaledTile.ok).toBe(true);
    if (!scaledTile.ok) return;
    expect(scaledTile.source).toMatch(
        /Image \{\n\s+x: 0px;\n\s+y: 0px;\n\s+width: parent\.width \/ 2;\n\s+height: parent\.height \/ 2;/u,
    );

    const rounded = convertSnapshotJson(
        JSON.stringify({
            ...image,
            root: { ...image.root, cornerRadii: [8, 8, 8, 8] },
        }),
    );
    expect(rounded.ok).toBe(true);
    if (!rounded.ok) return;
    expect(rounded.source).toContain("clip: true;");
    expect(rounded.source).toContain("border-radius: 8px;");
    expect(rounded.source).toMatch(/Rectangle \{\n\s+x: 0px;\n\s+clip: true;/u);
    expect(rounded.source).not.toContain("y: 0px;");
});

test("does not collapse root frame-like wrappers with any visual or layout semantics", async () => {
    const base = JSON.parse(
        await readFile("fixtures/default-heavy.snapshot.json", "utf8"),
    ) as { root: Record<string, unknown> };
    const root = {
        ...base.root,
        kind: "frame",
        containerKind: "native",
        fills: [],
        strokes: [],
        shadows: [],
        cornerRadii: [0, 0, 0, 0],
        clipsContent: false,
        opacity: 1,
        rotation: 0,
        autoLayout: null,
        children: [],
    };
    const collapsed = convertSnapshotJson(JSON.stringify({ ...base, root }));
    expect(collapsed.ok).toBe(true);
    if (!collapsed.ok) return;
    expect(collapsed.source.match(/Rectangle \{/gu)?.length ?? 0).toBe(0);
    for (const disqualifier of [
        {
            fills: [
                {
                    kind: "solid",
                    color: { r: 0, g: 0, b: 0, a: 1 },
                    opacity: 1,
                },
            ],
        },
        {
            strokes: [
                {
                    paint: {
                        kind: "solid",
                        color: { r: 0, g: 0, b: 0, a: 1 },
                        opacity: 1,
                    },
                    strokeTopWeight: 1,
                    strokeRightWeight: 1,
                    strokeBottomWeight: 1,
                    strokeLeftWeight: 1,
                    dashPattern: [],
                },
            ],
        },
        {
            shadows: [
                {
                    color: { r: 0, g: 0, b: 0, a: 1 },
                    offsetX: 0,
                    offsetY: 0,
                    blur: 2,
                    spread: 0,
                },
            ],
        },
        { cornerRadii: [4, 0, 0, 0] },
        { clipsContent: true },
        { opacity: 0.8 },
        { rotation: 2 },
        {
            autoLayout: {
                direction: "horizontal",
                paddingLeft: 0,
                paddingRight: 0,
                paddingTop: 0,
                paddingBottom: 0,
                itemSpacing: 0,
                wrap: false,
                counterAxisSpacing: 0,
                counterAxisAlignContent: "start",
                primaryAlignment: "start",
                counterAlignment: "center",
            },
        },
    ]) {
        const result = convertSnapshotJson(
            JSON.stringify({ ...base, root: { ...root, ...disqualifier } }),
        );
        expect(result.ok).toBe(true);
        if (!result.ok) continue;
        expect(
            result.source.match(/Rectangle \{/gu)?.length ?? 0,
        ).toBeGreaterThanOrEqual(1);
    }
});
