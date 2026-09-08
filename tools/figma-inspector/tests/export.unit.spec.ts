// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { requireValue } from "../src/preview/slint-ir";
import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import JSZip from "jszip";
import { convertCapture, convertExport } from "../src/preview/convert-capture";
import { unpackPreviewAssets } from "../src/asset-transport";
import { exportZip } from "../src/ui/export-download";
import { externalizeImages } from "../src/export/generate";
import { isExportPackage } from "../src/protocol";
import { isPluginToUiMessage } from "../src/protocol";

async function result(captureJson?: string) {
    const json =
        captureJson ??
        (await readFile("fixtures/source/export-fonts.json", "utf8"));
    const output = await convertCapture({
        type: "preview-capture",
        revision: 5,
        captureJson: json,
    });
    if (output.type !== "preview-source") throw Error(JSON.stringify(output));
    return {
        ...output,
        exportPackage: await convertExport({
            type: "preview-capture",
            revision: 5,
            captureJson: json,
        }),
    };
}

test("README lists a shared blend-mode limitation once across captured nodes", async () => {
    const capture = JSON.parse(
        await readFile("fixtures/source/export-fonts.json", "utf8"),
    );
    const text = capture.root.children[0];
    capture.root.children = Array.from({ length: 20 }, (_, i) => ({
        ...structuredClone(text),
        id: `text-${i}`,
        name: `Text ${i}`,
        properties: {
            ...text.properties,
            blendMode: i % 2 ? "SCREEN" : "MULTIPLY",
        },
    }));
    const output = await result(JSON.stringify(capture));
    const readme = output.exportPackage.files.find(
        (f) => f.path === "README.txt",
    )?.data;
    expect(readme?.match(/BLEND_MODE_APPROXIMATED:/g)).toHaveLength(1);
    expect(readme).toContain(
        "Non-normal blend modes are rendered with normal compositing",
    );
});

test.each(["HUG", "FILL"])(
    "rasterized %s text has fixed preview width and preferred native width in syntax and ZIP",
    async (sizing) => {
        const capture = JSON.parse(
            await readFile("fixtures/source/export-fonts.json", "utf8"),
        );
        Object.assign(capture.root.properties, {
            layoutMode: "HORIZONTAL",
            primaryAxisSizingMode: "FIXED",
            counterAxisSizingMode: "FIXED",
            primaryAxisAlignItems: "MIN",
            counterAxisAlignItems: "CENTER",
            paddingLeft: 0,
            paddingRight: 0,
            paddingTop: 0,
            paddingBottom: 0,
            itemSpacing: 8,
            layoutWrap: "NO_WRAP",
        });
        Object.assign(capture.root.children[0].properties, {
            layoutSizingHorizontal: sizing,
            layoutSizingVertical: "FIXED",
        });
        const output = await result(JSON.stringify(capture));
        const preview =
            typeof output.source === "string"
                ? output.source
                : unpackPreviewAssets(output.source).source;
        expect(preview).toContain("width: 70px;");
        expect(preview).not.toContain("preferred-width: 70px;");
        expect(preview).not.toContain('text: "Native export text"');
        const native = output.exportPackage.source;
        expect(native).toContain('text: "Native export text"');
        expect(native).toContain("preferred-width: 70px;");
        expect(native).not.toMatch(/(?<!preferred-)\bwidth: 70px;/);
        const zip = await JSZip.loadAsync(
            await exportZip(output.exportPackage),
        );
        expect(await requireValue(zip.file("main.slint")).async("string")).toBe(
            native,
        );
    },
);

test("one capture produces preview images and native export text with font imports and real assets", async () => {
    const output = await result();
    const preview =
        typeof output.source === "string"
            ? output.source
            : unpackPreviewAssets(output.source).source;
    expect(preview).not.toContain('text: "Native export text"');
    expect(preview).toContain("data:image/png;base64,");
    expect(output.exportPackage.source).toContain('text: "Native export text"');
    expect(output.exportPackage.source).toContain(
        '// import "fonts/roboto-regular.ttf";',
    );
    expect(output.exportPackage.source).not.toMatch(/^import "fonts\//m);
    expect(output.exportPackage.source).not.toContain("data:image");
    expect(
        output.exportPackage.files.filter((f) => f.path.startsWith("assets/")),
    ).toHaveLength(1);
    expect(isPluginToUiMessage(output)).toBe(true);
    expect((await result()).exportPackage).toEqual(output.exportPackage);
});

test("ZIP has exact syntax-view code, decodable image bytes and a font checklist, without font placeholders", async () => {
    const output = (await result()).exportPackage;
    const bytes = await exportZip(output);
    const zip = await JSZip.loadAsync(bytes);
    expect(await requireValue(zip.file("main.slint")).async("string")).toBe(
        output.source,
    );
    const fontReadme = await requireValue(zip.file("fonts/README.txt")).async(
        "string",
    );
    expect(fontReadme).toContain("Roboto / Regular");
    expect(fontReadme).toContain("commented out");
    expect(fontReadme).toContain("uncomment their import lines");
    expect(fontReadme).toContain("removing the leading //");
    expect(Object.keys(zip.files).some((name) => name.endsWith(".ttf"))).toBe(
        false,
    );
    const asset = requireValue(
        output.files.find((f) => f.path.endsWith(".png")),
    );
    expect(
        [
            ...(await requireValue(zip.file(asset.path)).async("uint8array")),
        ].slice(0, 8),
    ).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
    expect(await exportZip(output)).toEqual(bytes);
});

test("asset extraction deduplicates image expressions but never edits quoted code examples", () => {
    const image = '@image-url("data:image/png;base64,YWJj")';
    const source = `Text { text: ${JSON.stringify(image)}; } Image { source: ${image}; } Image { source: ${image}; }`;
    const result = externalizeImages(source);
    expect(result.files).toHaveLength(1);
    expect(result.source).toContain(JSON.stringify(image));
    expect(result.source.match(/assets\/image-/g)).toHaveLength(2);
});

test("export message validation rejects traversal, duplicate paths and malformed files", () => {
    const valid = {
        source: "export component Demo inherits Window {}",
        files: [{ path: "README.txt", encoding: "utf8", data: "hello" }],
    };
    expect(isExportPackage(valid)).toBe(true);
    for (const files of [
        [{ ...valid.files[0], path: "../main.slint" }],
        [valid.files[0], valid.files[0]],
        [null],
        [{ ...valid.files[0], encoding: "binary" }],
    ]) {
        expect(isExportPackage({ ...valid, files })).toBe(false);
        expect(
            isPluginToUiMessage({
                type: "preview-source",
                revision: 1,
                source: "",
                exportPackage: { ...valid, files },
            }),
        ).toBe(false);
    }
});
