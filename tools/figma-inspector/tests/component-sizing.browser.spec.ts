// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import { requireValue } from "../src/preview/slint-ir";
import type { SourceCapture, SourceNode } from "../src/plugin/source";
import { mountPreview, canvasPixels, readFixture } from "./browser-harness";

type Pixels = Awaited<ReturnType<typeof canvasPixels>>;
function bounds(pixels: Pixels, channel: number) {
    const points: [number, number][] = [];
    for (let y = 0; y < pixels.height; y++) {
        for (let x = 0; x < pixels.width; x++) {
            const i = (y * pixels.width + x) * 4;
            if (
                [0, 1, 2].every(
                    (c) => pixels.data[i + c] === (c === channel ? 255 : 0),
                )
            )
                points.push([x, y]);
        }
    }
    if (!points.length) return undefined;
    const xs = points.map(([x]) => x),
        ys = points.map(([, y]) => y);
    return {
        x: Math.min(...xs),
        y: Math.min(...ys),
        width: Math.max(...xs) - Math.min(...xs) + 1,
        height: Math.max(...ys) - Math.min(...ys) + 1,
    };
}
async function fixture(nested: boolean) {
    return JSON.parse(
        await readFixture(
            `fixtures/source/conditional-${nested ? "inner-row" : "root"}.json`,
        ),
    ) as SourceCapture;
}
async function generate(
    capture: SourceCapture,
    target: "preview" | "export",
    full = false,
) {
    const normalized = await normalizeSource(capture, target);
    if (!normalized.ok || normalized.empty)
        throw Error(JSON.stringify(normalized));
    const generated = convertSnapshot(normalized.snapshot, { target });
    if (!generated.ok) throw Error(JSON.stringify(generated));
    if (full) return generated.source;
    // Keep the generated public components; the test supplies an interactive host.
    return generated.source.slice(
        0,
        generated.source.lastIndexOf("export component Demo inherits Window"),
    );
}
function visit(node: SourceNode, apply: (node: SourceNode) => void) {
    apply(node);
    node.children?.forEach((child) => {
        visit(child, apply);
    });
}
for (const target of ["preview", "export"] as const) {
    for (const nested of [false, true]) {
        test(`${target}: conditional ${nested ? "inner rows" : "roots"} retain button geometry`, async () => {
            const p = await mountPreview();
            let revision = 0;
            for (const padding of [4, 6, 10]) {
                const capture = await fixture(nested);
                for (const variant of requireValue(capture.components)
                    .definitions[0].variants) {
                    variant.root.properties.paddingTop = padding;
                    variant.root.properties.paddingBottom = padding;
                    variant.root.properties.height = 20 + padding * 2;
                    variant.root.properties.primaryAxisAlignItems =
                        padding === 10 ? "CENTER" : "MIN";
                }
                const prefix = await generate(capture, target);
                const name = nested ? "NestedSizing" : "RootSizing";
                for (const position of ["leading", "trailing", "both"]) {
                    p.send({
                        type: "preview-source",
                        revision: ++revision,
                        source: `${prefix}
                        export component Geometry inherits Window {
                            width: 220px; height: 80px; background: white;
                            FlexboxLayout { alignment: start; cross-axis-alignment: start;
                                ${name} { ${nested ? "width: 160px;" : "horizontal-stretch: 0;"}
                                    vertical-stretch: 0; variant-position: ${name}Position.${position};
                                }
                            }
                        }`,
                    });
                    await p.ready(revision);
                    const pixels = await canvasPixels(p);
                    const contentWidth = position === "both" ? 95 : 71;
                    expect(
                        bounds(pixels, 2),
                        `${nested}/${padding}/${position} background`,
                    ).toEqual({
                        x: 0,
                        y: 0,
                        width: nested ? 160 : contentWidth + 24,
                        height: 20 + padding * 2,
                    });
                    if (nested) {
                        const rowX =
                            padding === 10 ? (160 - contentWidth) / 2 : 16;
                        const labelX =
                            rowX + (position === "trailing" ? 0 : 24);
                        expect(
                            Math.abs(
                                requireValue(bounds(pixels, 0)).x - labelX,
                            ),
                        ).toBeLessThanOrEqual(1);
                    }
                    expect(bounds(pixels, 0)?.y).toBe(padding);
                    expect(bounds(pixels, 0)?.height).toBe(20);
                    expect(bounds(pixels, 1)?.y).toBe(padding + 2);
                    expect(bounds(pixels, 1)?.height).toBe(16);
                }
            }
        }, 60000);
    }

    test(`${target}: Fill and wrapped families preserve allocated bounds`, async () => {
        const capture = await fixture(false);
        requireValue(
            capture.root.children,
        )[0].properties.layoutSizingHorizontal = "FILL";
        const p = await mountPreview();
        p.send({
            type: "preview-source",
            revision: 1,
            source: await generate(capture, target, true),
        });
        await p.ready(1);
        expect(bounds(await canvasPixels(p), 2)).toEqual({
            x: 0,
            y: 0,
            width: 300,
            height: 32,
        });
        const prefix = await generate(capture, target);
        p.send({
            type: "preview-source",
            revision: 2,
            source: `${prefix}
            export component Wrapped inherits Window {
                width:230px; height:130px; background:white;
                FlexboxLayout { flex-wrap:wrap; spacing:10px; spacing-vertical:8px;
                    alignment:start; cross-axis-alignment:start; cross-axis-line-alignment:start;
                    ${["leading", "trailing", "both", "leading", "trailing", "both"].map((position) => `RootSizing {horizontal-stretch:0;vertical-stretch:0;variant-position:RootSizingPosition.${position};}`).join("\n")}
                }
            }`,
        });
        await p.ready(2);
        const pixels = await canvasPixels(p);
        expect(bounds(pixels, 2)).toEqual({
            x: 0,
            y: 0,
            width: 224,
            height: 112,
        });
        for (const y of [6, 46, 86]) {
            let red = 0;
            for (let x = 0; x < pixels.width; x++) {
                const i = (y * pixels.width + x) * 4;
                if (
                    pixels.data[i] === 255 &&
                    pixels.data[i + 1] === 0 &&
                    pixels.data[i + 2] === 0
                )
                    red++;
            }
            expect(red).toBe(94);
        }
    });

    test(`${target}: Hug responds to label, icon and variant changes on the same instance`, async () => {
        const capture = await fixture(false);
        const text = (node: SourceNode) => {
            if (node.name !== "Label") return;
            node.type = "TEXT";
            Object.assign(node.properties, {
                characters: "Hi",
                fontName: { family: "Inter", style: "Regular" },
                fontSize: 14,
                fontWeight: 400,
                textAutoResize: "WIDTH_AND_HEIGHT",
                textAlignHorizontal: "LEFT",
                textAlignVertical: "TOP",
                lineHeight: { unit: "PIXELS", value: 20 },
                letterSpacing: { unit: "PIXELS", value: 0 },
                layoutSizingHorizontal: "HUG",
                layoutSizingVertical: "HUG",
            });
        };
        visit(capture.root, text);
        for (const variant of requireValue(capture.components).definitions[0]
            .variants) {
            visit(variant.root, text);
        }
        const prefix = await generate(capture, target);
        const p = await mountPreview();
        p.send({
            type: "preview-source",
            revision: 1,
            source: `${prefix}
            export component Interactive inherits Window {
                width: 320px; height: 80px; background: white;
                private property <int> step: 0;
                FlexboxLayout { alignment: start; cross-axis-alignment: start;
                    RootSizing { horizontal-stretch: 0; vertical-stretch: 0;
                        label: root.step == 0 ? "Hi" : "A much longer label";
                        icons: root.step != 2;
                        variant-position: root.step == 3 ? RootSizingPosition.both : RootSizingPosition.leading;
                    }
                }
                TouchArea { clicked => { root.step = mod(root.step + 1, 4); } }
            }`,
        });
        await p.ready(1);
        const widths: number[] = [];
        for (let step = 0; step < 4; step++) {
            if (step) {
                const canvas = p.element("#preview-canvas");
                canvas.style.pointerEvents = "auto";
                const rect = canvas.getBoundingClientRect();
                const Pointer = (p.win as Window & typeof globalThis)
                    .PointerEvent;
                for (const type of ["pointerdown", "pointerup"]) {
                    canvas.dispatchEvent(
                        new Pointer(type, {
                            bubbles: true,
                            pointerId: 1,
                            pointerType: "mouse",
                            isPrimary: true,
                            button: 0,
                            buttons: type === "pointerdown" ? 1 : 0,
                            clientX: rect.left + 10,
                            clientY: rect.top + 10,
                        }),
                    );
                }
                await expect
                    .poll(async () => bounds(await canvasPixels(p), 2)?.width)
                    .not.toBe(widths[step - 1]);
            }
            widths.push(requireValue(bounds(await canvasPixels(p), 2)).width);
        }
        expect(widths[1]).toBeGreaterThan(widths[0] + 50);
        expect(widths[1] - widths[2]).toBe(24);
        expect(widths[3] - widths[1]).toBe(24);
    });
}
