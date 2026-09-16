// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import type { SourceCapture } from "../src/plugin/source";

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
