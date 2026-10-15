// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import {
    hydrateBulkSource,
    indexBulkDocument,
} from "../src/plugin/bulk-source";
import type { SourceCapture } from "../src/plugin/source";
import { convertCapture } from "../src/preview/convert-capture";

const fixture = async () =>
    JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    ) as SourceCapture;

function compact(source: SourceCapture): SourceCapture {
    const bulkProperties = Object.keys(source.root.properties).filter(
        (key) => key !== "rotation",
    );
    const document = {
        id: source.root.id,
        type: source.root.type,
        ...source.root.properties,
    };
    return {
        ...source,
        bulkDocuments: [document],
        root: {
            ...source.root,
            bulkProperties,
            properties: { rotation: source.root.properties.rotation },
        },
    };
}

test("bulk supplements preserve preview, native export and lazy snapshots", async () => {
    const source = await fixture();
    const request = {
        type: "preview-capture" as const,
        revision: 1,
        captureJson: JSON.stringify(source),
    };
    const expected = await convertCapture(request);
    const smaller = compact(source);
    expect(hydrateBulkSource(smaller)).toEqual(source);
    const actual = await convertCapture({
        ...request,
        captureJson: JSON.stringify(smaller),
    });
    expect(actual.type).toBe("preview-source");
    if (actual.type !== "preview-source" || expected.type !== "preview-source")
        throw Error("Missing conversion");
    expect(actual.source).toEqual(expected.source);
    expect(actual.exportPackage).toEqual(expected.exportPackage);
    expect(actual.warnings).toEqual(expected.warnings);
    expect(smaller.root.properties).toEqual({ rotation: 0 });
});

test("worker reconstruction preserves instance types, child order and live overrides", async () => {
    const source = compact(await fixture());
    source.root.type = "INSTANCE";
    source.root.properties.width = 123;
    source.bulkDocuments![0].type = "FRAME";
    source.root.children = [
        {
            ...source.root,
            id: "second",
            type: "RECTANGLE",
            bulkProperties: undefined,
            children: undefined,
        },
        {
            ...source.root,
            id: "first",
            type: "RECTANGLE",
            bulkProperties: undefined,
            children: undefined,
        },
    ];
    const restored = hydrateBulkSource(source);
    expect(restored.root.type).toBe("INSTANCE");
    expect(restored.root.properties.width).toBe(123);
    expect(restored.root.children?.map((node) => node.id)).toEqual([
        "second",
        "first",
    ]);
});

test("owner documents are indexed even when an earlier root already contained their identity", async () => {
    const source = compact(await fixture());
    const original = source.bulkDocuments![0];
    source.bulkDocuments = [
        {
            id: "section",
            type: "SECTION",
            children: [{ ...original, width: 1 }],
        },
        original,
    ];
    expect(hydrateBulkSource(source).root.properties.width).toBe(100);
    const nodes = new Map<string, Record<string, unknown>>();
    const documents = new Set<string>();
    for (const document of source.bulkDocuments) {
        indexBulkDocument(document, nodes);
        documents.add(String(document.id));
    }
    expect(hydrateBulkSource(source, nodes, documents)).toEqual(
        hydrateBulkSource(source),
    );
});

test("missing or incompatible bulk metadata produces explicit conversion diagnostics", async () => {
    for (const corrupt of [
        (source: SourceCapture) => {
            source.bulkDocuments = [];
        },
        (source: SourceCapture) => {
            source.bulkDocuments![0].type = "TEXT";
        },
        (source: SourceCapture) => {
            source.root.bulkProperties = ["fontName"];
        },
        (source: SourceCapture) => {
            source.bulkDocuments![0].children = [null];
        },
    ]) {
        const source = compact(await fixture());
        corrupt(source);
        const result = await convertCapture({
            type: "preview-capture",
            revision: 1,
            captureJson: JSON.stringify(source),
        });
        expect(result).toMatchObject({
            type: "preview-diagnostics",
            diagnostics: [{ code: "CAPTURE_CONVERSION_ERROR" }],
        });
    }
});

test("shared property sets reconstruct identically and reject invalid references", async () => {
    const source = compact(await fixture());
    source.bulkPropertySets = [[...source.root.bulkProperties!]];
    source.root.bulkPropertySet = 0;
    delete source.root.bulkProperties;
    expect(hydrateBulkSource(source)).toEqual(await fixture());
    source.root.bulkPropertySet = 2;
    expect(() => hydrateBulkSource(source)).toThrow(
        "Invalid bulk property set",
    );
});

test("omitted REST paint opacity retains the fill in preview and native export", async () => {
    const source = await fixture();
    const expected = await convertCapture({
        type: "preview-capture",
        revision: 1,
        captureJson: JSON.stringify(source),
    });
    const smaller = compact(source);
    const fills = smaller.bulkDocuments![0].fills as Record<string, unknown>[];
    delete fills[0].opacity;
    const actual = await convertCapture({
        type: "preview-capture",
        revision: 1,
        captureJson: JSON.stringify(smaller),
    });
    if (actual.type !== "preview-source" || expected.type !== "preview-source")
        throw Error("Missing conversion");
    expect(actual.source).toEqual(expected.source);
    expect(actual.exportPackage).toEqual(expected.exportPackage);
    expect(actual.warnings).not.toContainEqual(
        expect.objectContaining({ code: "UNSUPPORTED_PAINT" }),
    );
});
