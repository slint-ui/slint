// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { canvasPixels, mountPreview, readFixture } from "./browser-harness";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";

for (const file of [
    "conditional-root",
    "conditional-inner-row",
    "component-raster-icon",
]) {
    test(`${file}: specialized pixels equal reusable pixels`, async () => {
        const normalized = await normalizeSource(
            JSON.parse(await readFixture(`fixtures/source/${file}.json`)),
        );
        if (!normalized.ok || normalized.empty) throw Error("Invalid fixture");
        const p = await mountPreview();
        const pixels: Awaited<ReturnType<typeof canvasPixels>>[] = [];
        for (const specialize of [false, true]) {
            const converted = convertSnapshot(normalized.snapshot, {
                specialize,
            });
            if (!converted.ok) throw Error(JSON.stringify(converted));
            const revision = pixels.length + 1;
            p.send({
                type: "preview-source",
                revision,
                source: converted.source,
            });
            await p.ready(revision);
            pixels.push(await canvasPixels(p));
        }
        expect(pixels[1].width).toBe(pixels[0].width);
        expect(pixels[1].height).toBe(pixels[0].height);
        let different = 0;
        for (let i = 0; i < pixels[0].data.length; i++)
            if (pixels[0].data[i] !== pixels[1].data[i]) different++;
        expect(different).toBe(0);
    });
}

test("render source is internal; copy stays reusable and failed or stale revisions cannot publish it", async () => {
    const p = await mountPreview();
    const source =
        "export component Readable inherits Window { width: 50px; height: 40px; background: red; }";
    const renderSource =
        "export component Specialized inherits Window { width: 50px; height: 40px; background: blue; }";
    p.send({ type: "preview-source", revision: 1, source, renderSource });
    await p.ready(1);
    const pixels = await canvasPixels(p);
    expect([...pixels.data.slice(0, 4)]).toEqual([0, 0, 255, 255]);
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
        source,
        renderSource: "invalid Slint",
    });
    await expect.poll(() => p.element("#status").dataset.state).toBe("error");
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(true);
    p.send({ type: "preview-source", revision: 1, source, renderSource });
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(true);
});
