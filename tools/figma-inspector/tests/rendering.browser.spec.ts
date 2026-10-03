// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { convertSnapshotJson, convertSnapshot } from "../src/preview/converter";
import { normalizeSource } from "../src/plugin/normalize";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import type { SourceCapture } from "../src/plugin/source";
import { styledTextFamily } from "./button-family";

test.each([
    ["W", 24, false],
    ["Wide text", 200, false],
    ["Å", 24, false],
    ["e\u0301", 24, false],
    ["Д", 24, false],
    ["Two\nlines", 160, true],
    ["A much longer label that wraps", 100, true],
] as const)(
    "generic text overflow preserves glyph positions: %s",
    async (characters, width, wrapped) => {
        const capture: SourceCapture = JSON.parse(
            await readFixture("fixtures/source/export-fonts.json"),
        );
        const label = capture.root.children?.[0];
        if (!label) throw Error("Expected text fixture");
        Object.assign(capture.root.properties, { width: 280, height: 180 });
        capture.root.children = [label];
        Object.assign(label.properties, {
            x: (280 - width) / 2,
            y: 78,
            width,
            height: 24,
            characters,
            fontName: { family: "Inter", style: "Regular" },
            fontSize: 36,
            fontWeight: 400,
            textAutoResize: "NONE",
            textAlignHorizontal: "CENTER",
            textAlignVertical: "CENTER",
            fills: [{ type: "SOLID", color: { r: 0, g: 0, b: 0 }, opacity: 1 }],
            textPaintBounds: { x: -8, y: -16, width: width + 16, height: 64 },
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
        width: 280px; height: 180px; background: white;
        Text { x: ${(280 - (wrapped ? width : 240)) / 2}px; y: -60px; width: ${wrapped ? width : 240}px; height: 300px;
            text: ${JSON.stringify(characters)}; font-family: "Inter"; font-size: 36px; font-weight: 400; color: black;
            ${wrapped ? "wrap: word-wrap;" : ""}
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
    },
);

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

test("styled component families compile through the built interpreter", async () => {
    const capture = styledTextFamily(
        JSON.parse(
            await readFixture("fixtures/source/component-variants.json"),
        ) as SourceCapture,
    );
    const normalized = await normalizeSource(capture);
    if (!normalized.ok || normalized.empty)
        throw Error("Cannot normalize styled component family");
    const result = convertSnapshot(normalized.snapshot);
    if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: result.source,
        exportPackage: { source: result.source, files: [] },
    });
    await p.ready(1);
});

test("component families without property contracts compile for preview and export", async () => {
    const capture = JSON.parse(
        await readFixture("fixtures/source/component-variants.json"),
    ) as SourceCapture;
    for (const definition of capture.components?.definitions ?? [])
        delete definition.contract;
    const p = await mountPreview();
    let revision = 0;
    for (const target of ["preview", "export"] as const) {
        const normalized = await normalizeSource(capture, target);
        if (!normalized.ok || normalized.empty)
            throw Error(`Cannot normalize contract-free ${target} family`);
        const result = convertSnapshot(normalized.snapshot, { target });
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
