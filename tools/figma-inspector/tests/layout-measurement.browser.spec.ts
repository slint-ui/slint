// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import type { SourceCapture } from "../src/plugin/source";
import { requireValue } from "../src/preview/slint-ir";

test("nested HUG containers use resolved preview geometry", async () => {
    let referencePixels: Awaited<ReturnType<typeof canvasPixels>> | undefined;
    for (const specialize of [false, true]) {
        const capture: SourceCapture = JSON.parse(
            await readFixture("fixtures/source/conditional-root.json"),
        );
        const definition = requireValue(capture.components).definitions[0];
        delete definition.contract;
        for (const variant of definition.variants) {
            const fixed = requireValue(variant.root.children)[0];
            Object.assign(fixed.properties, {
                width: 95,
                height: 84,
                layoutSizingHorizontal: "FIXED",
                layoutSizingVertical: "FIXED",
            });
            const inner = {
                ...structuredClone(variant.root),
                id: `${variant.id}:inner`,
                name: "Inner HUG container",
                type: "FRAME",
                properties: {
                    ...variant.root.properties,
                    width: 95,
                    height: 84,
                    fills: [],
                    layoutMode: "VERTICAL",
                    layoutSizingHorizontal: "HUG",
                    layoutSizingVertical: "HUG",
                    paddingLeft: 0,
                    paddingRight: 0,
                    paddingTop: 0,
                    paddingBottom: 0,
                    itemSpacing: 0,
                },
                children: [fixed],
            };
            Object.assign(variant.root.properties, {
                width: 95,
                height: 84,
                layoutMode: "VERTICAL",
                layoutSizingHorizontal: "HUG",
                layoutSizingVertical: "HUG",
                paddingLeft: 0,
                paddingRight: 0,
                paddingTop: 0,
                paddingBottom: 0,
                itemSpacing: 0,
            });
            variant.root.children = [inner];
        }
        const instance = requireValue(capture.root.children)[0];
        const id = instance.id;
        Object.assign(instance, structuredClone(definition.variants[0].root), {
            id,
            type: "INSTANCE",
        });
        const normalized = await normalizeSource(capture, "preview");
        if (!normalized.ok || normalized.empty)
            throw Error("Expected nested HUG containers");
        const result = convertSnapshot(normalized.snapshot, { specialize });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        expect(result.source).toContain("height: 84px;");
        const p = await mountPreview();
        p.send({
            type: "preview-source",
            revision: 1,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(1);
        const pixels = await canvasPixels(p);
        if (referencePixels) expect(pixels.data).toEqual(referencePixels.data);
        else referencePixels = pixels;
    }
});

for (const specialize of [false, true]) {
    test(`nested HUG image layouts render (${specialize ? "specialized" : "reusable"})`, async () => {
        const capture: SourceCapture = JSON.parse(
            await readFixture("fixtures/source/conditional-root.json"),
        );
        capture.pngEnabled = false;
        const definition = requireValue(capture.components).definitions[0];
        delete definition.contract;
        for (const variant of definition.variants) {
            const frame = (suffix: string, direction = "VERTICAL") => ({
                id: `${variant.id}:${suffix}`,
                name: suffix,
                type: "FRAME",
                errors: [],
                properties: {
                    ...variant.root.properties,
                    width: 74,
                    height: 40,
                    fills: [],
                    layoutMode: direction,
                    layoutSizingHorizontal: "HUG",
                    layoutSizingVertical: "HUG",
                    paddingLeft: 0,
                    paddingRight: 0,
                    paddingTop: 0,
                    paddingBottom: 0,
                    itemSpacing: 0,
                },
                children: [] as NonNullable<SourceCapture["root"]["children"]>,
            });
            const middle = frame("middle");
            const row = frame("row", "HORIZONTAL");
            Object.assign(row.properties, {
                layoutSizingVertical: "FIXED",
                paddingLeft: 16,
                paddingRight: 16,
            });
            const content = frame("content", "HORIZONTAL");
            content.properties.width = 42;
            content.properties.height = 20;
            const image = frame("image");
            image.type = "VECTOR";
            Object.assign(image.properties, {
                width: 42,
                height: 20,
                layoutMode: "NONE",
            });
            content.children = [
                {
                    ...image,
                    exports: {
                        svg: {
                            value: '<svg xmlns="http://www.w3.org/2000/svg" width="42" height="20"><path fill="#ff0000" d="M0 0h42v20H0z"/></svg>',
                        },
                        png: {},
                    },
                },
            ];
            row.children = [content];
            middle.children = [row];
            Object.assign(variant.root, frame("outer"), {
                id: variant.id,
                type: "COMPONENT",
                children: [middle],
            });
        }
        const instance = requireValue(capture.root.children)[0];
        const id = instance.id;
        Object.assign(instance, structuredClone(definition.variants[0].root), {
            id,
            type: "INSTANCE",
        });
        const normalized = await normalizeSource(capture, "preview");
        if (!normalized.ok || normalized.empty)
            throw Error("Expected HUG layouts");
        const result = convertSnapshot(normalized.snapshot, { specialize });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        expect(
            result.source.match(/FlexboxLayout/g)?.length,
        ).toBeGreaterThanOrEqual(4);
        const p = await mountPreview();
        p.send({
            type: "preview-source",
            revision: 1,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(1);
        expect(result.source).toContain("width: 42px;");
        const pixels = await canvasPixels(p);
        let red = 0;
        for (let i = 0; i < pixels.data.length; i += 4)
            if (
                pixels.data[i] === 255 &&
                pixels.data[i + 1] === 0 &&
                pixels.data[i + 2] === 0
            )
                red++;
        expect(red).toBe(42 * 20);
    });
}

test("nested fixed layouts render without geometry recovery", async () => {
    const capture: SourceCapture = JSON.parse(
        await readFixture("fixtures/source/negative-gap.json"),
    );
    Object.assign(capture.root.properties, {
        width: 100,
        height: 60,
        itemSpacing: 0,
        layoutSizingHorizontal: "FIXED",
        layoutSizingVertical: "FIXED",
    });
    let parent = capture.root;
    for (let depth = 1; depth < 20; depth++) {
        const child = structuredClone(capture.root);
        child.id = `nested:${depth}`;
        child.children = [];
        parent.children = [child];
        parent = child;
    }
    // Export normalization retains every native layout for this reproduction.
    const normalized = await normalizeSource(capture, "export");
    if (!normalized.ok || normalized.empty)
        throw Error("Expected nested layouts");
    const result = convertSnapshot(normalized.snapshot);
    if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
    expect(result.source.match(/FlexboxLayout/g)).toHaveLength(19);
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: result.source,
        exportPackage: { source: result.source, files: [] },
    });
    await p.ready(1);
    const pixels = await canvasPixels(p);
    expect(pixels.width).toBe(100);
    expect(pixels.height).toBe(60);
});
