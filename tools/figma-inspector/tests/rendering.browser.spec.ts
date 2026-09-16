// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { convertSnapshotJson, convertSnapshot } from "../src/preview/converter";
import { normalizeSource } from "../src/plugin/normalize";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import type { SourceCapture } from "../src/plugin/source";

test("generic text overflow preserves the same glyph position as a roomy text box", async () => {
    const capture: SourceCapture = JSON.parse(
        await readFixture("fixtures/source/export-fonts.json"),
    );
    const label = capture.root.children?.[0];
    if (!label) throw Error("Expected text fixture");
    Object.assign(capture.root.properties, { width: 140, height: 100 });
    capture.root.children = [label];
    Object.assign(label.properties, {
        x: 68,
        y: 53,
        width: 24,
        height: 24,
        characters: "W",
        fontName: { family: "Inter", style: "Regular" },
        fontSize: 36,
        fontWeight: 400,
        textAutoResize: "NONE",
        textAlignHorizontal: "CENTER",
        textAlignVertical: "CENTER",
        fills: [{ type: "SOLID", color: { r: 0, g: 0, b: 0 }, opacity: 1 }],
        textPaintBounds: { x: -6, y: -6, width: 36, height: 36 },
    });
    const normalized = await normalizeSource(capture, "export");
    if (!normalized.ok || normalized.empty)
        throw Error("Expected text snapshot");
    const generated = convertSnapshot(normalized.snapshot, {
        target: "export",
    });
    if (!generated.ok) throw Error("Expected native source");
    expect(generated.source).toContain("self.preferred-width");
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: generated.source,
        exportPackage: { source: generated.source, files: [] },
    });
    await p.ready(1);
    const actual = await canvasPixels(p);
    const reference = `export component Reference inherits Window {
        width: 140px; height: 100px; background: white;
        Text { x: 50px; y: 30px; width: 60px; height: 70px;
            text: "W"; font-family: "Inter"; font-size: 36px; font-weight: 400; color: black;
            horizontal-alignment: center; vertical-alignment: center;
        }
    }`;
    p.send({
        type: "preview-source",
        revision: 2,
        source: reference,
        exportPackage: { source: reference, files: [] },
    });
    await p.ready(2);
    const expected = await canvasPixels(p);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    expect(
        actual.data.reduce(
            (count, value, index) =>
                count + Number(value !== expected.data[index]),
            0,
        ),
    ).toBe(0);
});

test("authored layouts, images and component families compile through the built interpreter", async () => {
    const p = await mountPreview();
    let revision = 0;
    for (const name of [
        "button",
        "auto-layout",
        "hug-button",
        "nested-fill",
        "nested-instance",
        "image-fill",
        "svg-multi-path",
    ]) {
        const result = convertSnapshotJson(
            await readFixture(`fixtures/${name}.snapshot.json`),
        );
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(revision);
    }
    for (const name of [
        "component-intrinsic",
        "component-variants",
        "component-tokens",
        "empty-flex-spacers",
        "group-positioning",
        "image-crop",
    ]) {
        const normalized = await normalizeSource(
            JSON.parse(await readFixture(`fixtures/source/${name}.json`)),
        );
        if (!normalized.ok || normalized.empty)
            throw Error(`Cannot normalize ${name}`);
        const result = convertSnapshot(normalized.snapshot);
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: result.source,
            exportPackage: { source: result.source, files: [] },
        });
        await p.ready(revision);
    }
});

test("root-only snippets compile with native text and appearance helpers without font files", async () => {
    const p = await mountPreview();
    let revision = 0;
    for (const name of [
        "button",
        "frame",
        "auto-layout",
        "nested-instance",
        "image-fill",
        "svg-multi-path",
        "component-text",
    ]) {
        const snapshot = JSON.parse(
            await readFixture(`fixtures/${name}.snapshot.json`),
        );
        const result = convertSnapshot(snapshot, { scope: "root-only" });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
            exportPackage: {
                source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
                files: [],
            },
        });
        await p.ready(revision);
    }
    for (const name of [
        "translucent-styled-text",
        "inner-shadow",
        "asymmetric-rounded-stroke",
        "reverse-paint-order",
    ]) {
        const source = JSON.parse(
            await readFixture(`fixtures/source/${name}.json`),
        );
        if (name === "translucent-styled-text")
            source.root = source.root.children[0];
        const normalized = await normalizeSource(source, "export");
        if (!normalized.ok || normalized.empty)
            throw Error(`Cannot normalize ${name}`);
        const result = convertSnapshot(normalized.snapshot, {
            scope: "root-only",
        });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
            exportPackage: {
                source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
                files: [],
            },
        });
        await p.ready(revision);
    }
    // The generated HTML must not fetch sibling scripts, styles or WASM.
    const resources = p.win.performance.getEntriesByType("resource");
    expect(resources.filter((entry) => /^https?:/.test(entry.name))).toEqual(
        [],
    );
});
