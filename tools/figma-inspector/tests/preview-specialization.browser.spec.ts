// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    type CanvasPixels,
    expectCanvasPixels,
    mountCanvasPreview,
    readFixture,
} from "./browser-harness";
import type { SourceCapture } from "../src/plugin/source";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import { repeatedPreview } from "./repeated-preview";

test.each([false, true])(
    "repeated native effects preserve shared preview values (border: %s)",
    async (border) => {
        const capture = repeatedPreview(
            JSON.parse(
                await readFixture("fixtures/source/conditional-root.json"),
            ),
            8,
        );
        for (const [index, node] of (capture.root.children ?? []).entries()) {
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
            if (border) {
                Object.assign(node.properties, {
                    strokes: [
                        {
                            type: "SOLID",
                            opacity: 0.5,
                            color: { r: 1, g: 0, b: 0 },
                        },
                    ],
                    strokeWeight: 3,
                    strokeTopWeight: 3,
                    strokeRightWeight: 3,
                    strokeBottomWeight: 3,
                    strokeLeftWeight: 3,
                    strokeAlign: "INSIDE",
                    opacity: 0.7 + index * 0.04,
                    clipsContent: index % 2 === 0,
                });
                node.properties.effects.push({
                    type: "DROP_SHADOW",
                    visible: true,
                    radius: 4 + index,
                    spread: index - 2,
                    offset: { x: index, y: -index },
                    color: { r: index / 7, g: 0, b: 1, a: 0.8 },
                    blendMode: "NORMAL",
                    showShadowBehindNode: false,
                });
            }
        }
        const normalized = await normalizeSource(capture, "export");
        if (!normalized.ok || normalized.empty)
            throw Error(JSON.stringify(normalized));
        const p = await mountCanvasPreview();
        let reference: CanvasPixels | undefined;
        for (const specialize of [false, true]) {
            const converted = convertSnapshot(normalized.snapshot, {
                specialize,
            });
            if (!converted.ok) throw Error(JSON.stringify(converted));
            const revision = specialize ? 2 : 1;
            p.send({
                type: "preview-source",
                revision,
                source: converted.source,
                exportPackage: { source: converted.source, files: [] },
            });
            await p.ready(revision);
            const pixels = await expectCanvasPixels(p, (actual) => {
                if (reference) expect(actual.data).toEqual(reference.data);
                else
                    expect(
                        actual.data.some(
                            (value, index) => index % 4 !== 3 && value < 200,
                        ),
                    ).toBe(true);
            });
            reference ??= pixels;
        }
    },
);

for (const file of [
    "conditional-root",
    "conditional-inner-row",
    "component-raster-icon",
]) {
    test(`${file}: specialized pixels equal reusable pixels`, async () => {
        const capture: SourceCapture = JSON.parse(
            await readFixture(`fixtures/source/${file}.json`),
        );
        const colors =
            file === "component-raster-icon"
                ? [[40, 120, 200, 255]]
                : [
                      [255, 0, 0, 255],
                      [0, 255, 0, 255],
                  ];
        const normalized = await normalizeSource(capture);
        if (!normalized.ok || normalized.empty) throw Error("Invalid fixture");
        const p = await mountCanvasPreview();
        let reference: CanvasPixels | undefined;
        for (const specialize of [false, true]) {
            const converted = convertSnapshot(normalized.snapshot, {
                specialize,
            });
            if (!converted.ok) throw Error(JSON.stringify(converted));
            const revision = specialize ? 2 : 1;
            p.send({
                type: "preview-source",
                revision,
                source: converted.source,
                exportPackage: { source: converted.source, files: [] },
            });
            await p.ready(revision);
            const pixels = await expectCanvasPixels(p, (actual) => {
                for (const color of colors) {
                    let count = 0;
                    for (let index = 0; index < actual.data.length; index += 4)
                        if (
                            color.every(
                                (value, channel) =>
                                    actual.data[index + channel] === value,
                            )
                        )
                            count++;
                    if (file === "component-raster-icon")
                        expect(count).toBe(24 * 24);
                    else expect(count).toBeGreaterThan(0);
                }
                if (reference) {
                    expect(actual.width).toBe(reference.width);
                    expect(actual.height).toBe(reference.height);
                    expect(actual.data).toEqual(reference.data);
                }
            });
            reference ??= pixels;
        }
    });
}

test("render source is internal; copy stays reusable and failed or stale revisions cannot publish it", async () => {
    const p = await mountCanvasPreview();
    const source =
        "export component Readable inherits Window { width: 50px; height: 40px; background: red; }";
    const renderSource =
        "export component Specialized inherits Window { width: 50px; height: 40px; background: blue; }";
    p.send({
        type: "preview-source",
        revision: 1,
        source: renderSource,
        exportPackage: { source: source, files: [] },
    });
    await p.ready(1);
    await expectCanvasPixels(p, (pixels) => {
        expect([...pixels.data.slice(0, 4)]).toEqual([0, 0, 255, 255]);
    });
    await expect.poll(() => p.element("#source-view").textContent).toBe(source);
    const copied: string[] = [];
    p.doc.execCommand = () => {
        copied.push((p.doc.activeElement as HTMLTextAreaElement).value);
        return true;
    };
    p.element<HTMLButtonElement>("#copy-button").click();
    await expect.poll(() => copied).toEqual([source]);
    p.send({
        type: "preview-source",
        revision: 2,
        source: "invalid Slint",
        exportPackage: { source: source, files: [] },
    });
    await expect.poll(() => p.element("#status").dataset.state).toBe("error");
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(true);
    p.send({
        type: "preview-source",
        revision: 1,
        source: renderSource,
        exportPackage: { source: source, files: [] },
    });
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(true);
});
