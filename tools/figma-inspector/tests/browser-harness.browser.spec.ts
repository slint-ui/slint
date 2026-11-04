// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import {
    canvasPixels,
    decodePng,
    expectCanvasPixels,
    mountCanvasPreview,
    readFixture,
    type Preview,
    type CanvasPixels,
} from "./browser-harness";
import { compareScreenshotPixels } from "./screenshot-compare";

function deferImageLoads(preview: Preview) {
    const browser = preview.win as Window & typeof globalThis;
    const prototype = browser.HTMLImageElement.prototype;
    const own = Object.getOwnPropertyDescriptor(prototype, "onload");
    const native = Object.getOwnPropertyDescriptor(
        browser.HTMLElement.prototype,
        "onload",
    );
    if (!native?.set) throw Error("Missing native image load handler");
    const pending: (() => void)[] = [];
    let released = false;
    Object.defineProperty(prototype, "onload", {
        configurable: true,
        get: native.get,
        set(handler: (this: HTMLImageElement, event: Event) => void) {
            native.set?.call(
                this,
                function (this: HTMLImageElement, event: Event) {
                    const deliver = () => handler.call(this, event);
                    if (released) deliver();
                    else pending.push(deliver);
                },
            );
        },
    });
    return {
        pending: () => pending.length,
        release() {
            released = true;
            for (const deliver of pending.splice(0)) deliver();
        },
        restore() {
            if (own) Object.defineProperty(prototype, "onload", own);
            else Reflect.deleteProperty(prototype, "onload");
            pending.splice(0);
        },
    };
}

async function fixture(preview: Preview, kind: "SVG" | "PNG") {
    const path =
        kind === "SVG"
            ? "fixtures/source/svg-paint-bounds"
            : `fixtures/authored/raster/asymmetric-${preview.win.devicePixelRatio}x`;
    const normalized = await normalizeSource(
        JSON.parse(await readFixture(`${path}.json`)),
    );
    if (!normalized.ok || normalized.empty)
        throw Error("Invalid image fixture");
    const converted = convertSnapshot(normalized.snapshot);
    if (!converted.ok) throw Error(JSON.stringify(converted.diagnostics));
    const expected =
        kind === "PNG"
            ? await decodePng(
                  await server.commands.readFile(`${path}.png`, "base64"),
              )
            : undefined;
    const check = (pixels: CanvasPixels) => {
        if (expected) {
            expect(
                compareScreenshotPixels(expected, pixels, 2).differingPixels,
                "PNG image pixels",
            ).toBe(0);
        } else {
            const scale = preview.win.devicePixelRatio;
            expect(pixels.width).toBe(80 * scale);
            expect(pixels.height).toBe(60 * scale);
            const colors = [
                [16, 18, [255, 0, 0, 255]],
                [24, 24, [0, 0, 255, 255]],
                [48, 39, [255, 0, 0, 255]],
                [14, 18, [255, 255, 255, 255]],
            ] as const;
            for (const [x, y, color] of colors) {
                const offset =
                    (Math.floor(y * scale) * pixels.width +
                        Math.floor(x * scale)) *
                    4;
                expect(
                    Array.from(pixels.data.slice(offset, offset + 4)),
                    "SVG image pixels",
                ).toEqual(color);
            }
        }
    };
    preview.send({
        type: "preview-source",
        revision: 1,
        source: converted.source,
        exportPackage: { source: converted.source, files: [] },
    });
    await preview.ready(1);
    return check;
}

function geometry(preview: Preview) {
    const canvas = preview.element<HTMLCanvasElement>("#preview-canvas");
    return {
        iframe: preview.iframe.style.cssText,
        viewport: [preview.win.innerWidth, preview.win.innerHeight],
        canvas: canvas.getBoundingClientRect().toJSON(),
        buffer: [canvas.width, canvas.height],
    };
}

for (const kind of ["SVG", "PNG"] as const) {
    test(`${kind} pixel assertions wait for delayed image loading without resizing`, async () => {
        const preview = await mountCanvasPreview();
        const loads = deferImageLoads(preview);
        try {
            const check = await fixture(preview, kind);
            await expect.poll(loads.pending).toBeGreaterThan(0);
            expect(
                (await canvasPixels(preview)).data.every(
                    (value) => value === 255,
                ),
            ).toBe(true);
            const before = geometry(preview);
            let attempts = 0;
            let settled = false;
            const result = expectCanvasPixels(preview, (pixels) => {
                attempts++;
                check(pixels);
            }).then(
                () => {
                    settled = true;
                    return undefined;
                },
                (error: unknown) => {
                    settled = true;
                    return error;
                },
            );
            await expect.poll(() => attempts).toBeGreaterThanOrEqual(2);
            expect(settled).toBe(false);
            expect(geometry(preview)).toEqual(before);
            loads.release();
            expect(await result).toBeUndefined();
            expect(geometry(preview)).toEqual(before);
        } finally {
            loads.restore();
        }
    });

    test(`${kind} pixel assertions fail when image loading exceeds the deadline`, async () => {
        const preview = await mountCanvasPreview();
        const loads = deferImageLoads(preview);
        try {
            const check = await fixture(preview, kind);
            await expect.poll(loads.pending).toBeGreaterThan(0);
            let mismatch: unknown;
            const result = await expectCanvasPixels(
                preview,
                (pixels) => {
                    try {
                        check(pixels);
                    } catch (error) {
                        mismatch = error;
                        throw error;
                    }
                },
                { timeout: 3000 },
            ).then(
                () => undefined,
                (error: unknown) => error,
            );
            expect(mismatch).toBeInstanceOf(Error);
            expect(result).toBe(mismatch);
            if (result instanceof Error) {
                expect(result.message).toContain(`${kind} image pixels`);
                expect(result.message).toContain("Final canvas screenshot:");
            }
            loads.release();
            await expectCanvasPixels(preview, check);
        } finally {
            loads.restore();
        }
    });
}
