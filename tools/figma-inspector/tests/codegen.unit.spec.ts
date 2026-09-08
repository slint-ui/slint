// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { captureSource } from "../src/plugin/capture";
import { normalizeSource } from "../src/plugin/normalize";
import { generateCodegen } from "../src/plugin/codegen";
import { convertSnapshot } from "../src/preview/converter";

test("root-only conversion ignores even malformed descendants without mutating the input", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    );
    const full = convertSnapshot(snapshot);
    const root = convertSnapshot(snapshot, { scope: "root-only" });
    expect(full.ok).toBe(true);
    expect(root.ok).toBe(true);
    if (!root.ok || !full.ok) return;
    expect(root.source).not.toContain("export component");
    expect(root.source).not.toBe(full.source);
    const original = JSON.stringify(snapshot);
    const poisoned = {
        ...snapshot,
        components: { invalid: true },
        root: {
            ...snapshot.root,
            children: [{ kind: "not-supported", name: "Never emit me" }],
        },
    };
    expect(convertSnapshot(poisoned, { scope: "root-only" })).toEqual(root);
    expect(convertSnapshot(poisoned).ok).toBe(false);
    expect(JSON.stringify(snapshot)).toBe(original);
    expect(
        convertSnapshot(
            { ...snapshot, root: { ...snapshot.root, width: -1 } },
            { scope: "root-only" },
        ).ok,
    ).toBe(false);
});

test.each([
    "button",
    "auto-layout",
    "nested-instance",
    "image-fill",
    "svg-multi-path",
])("root-only %s retains root geometry and is deterministic", async (name) => {
    const snapshot = JSON.parse(
        await readFile(`fixtures/${name}.snapshot.json`, "utf8"),
    );
    snapshot.root.x = 37;
    snapshot.root.y = 49;
    const result = convertSnapshot(snapshot, { scope: "root-only" });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.source).toContain("x: 37px;");
    expect(result.source).toContain("y: 49px;");
    expect(result.source).not.toMatch(/export component|import .*fonts\//);
    expect(convertSnapshot(snapshot, { scope: "root-only" })).toEqual(result);
});

test.each(["FRAME", "COMPONENT", "COMPONENT_SET", "INSTANCE", "GROUP"])(
    "root-only capture never traverses or exports a %s subtree",
    async (type) => {
        const fixture = JSON.parse(
            await readFile("fixtures/source/basic.json", "utf8"),
        );
        const node = {
            ...fixture.root.properties,
            id: "root",
            name: "Root",
            type,
            get children() {
                throw Error("Descendants were accessed");
            },
            getMainComponentAsync() {
                throw Error("Component expanded");
            },
        } as unknown as SceneNode;
        const forbidden = async (): Promise<never> => {
            throw Error("Subtree exported");
        };
        const captured = await captureSource(
            node,
            Symbol(),
            forbidden,
            undefined,
            forbidden,
            1,
            false,
            undefined,
            undefined,
            4,
            false,
            undefined,
            undefined,
            false,
            "root-only",
        );
        expect(captured.work).toMatchObject({
            capturedNodes: 1,
            componentFamilies: 0,
            componentVariants: 0,
            svgExports: 0,
            pngExports: 0,
        });
        expect(captured.source.root.children).toEqual([]);
        expect(captured.source.root.errors).toEqual([]);
        expect(captured.source.components).toBeUndefined();
        const result = await generateCodegen(node, Symbol());
        expect(result[0]).toMatchObject({ language: "CSS" });
    },
);

test("codegen returns standalone root code and recovers after an invalid request", async () => {
    const fixture = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    const node = {
        ...fixture.root.properties,
        id: "root",
        name: "Root",
        type: "RECTANGLE",
    } as SceneNode;
    const failed = await generateCodegen({ ...node, width: -1 }, Symbol());
    expect(failed).toEqual([
        expect.objectContaining({
            title: "Diagnostics",
            language: "PLAINTEXT",
        }),
    ]);
    const [first, second] = await Promise.all([
        generateCodegen(node, Symbol()),
        generateCodegen({ ...node, name: "Other", width: 211 }, Symbol()),
    ]);
    expect(first[0]).toMatchObject({
        title: "Slint Code: Root",
        language: "CSS",
    });
    expect(first[0].code).toContain("width: 100px;");
    expect(second[0].code).toContain("width: 211px;");
    expect(second[0].title).toBe("Slint Code: Other");
});

test("native styled text does not require font export APIs", async () => {
    const fixture = JSON.parse(
        await readFile("fixtures/source/translucent-styled-text.json", "utf8"),
    );
    const text = fixture.root.children[0];
    const node = {
        ...text.properties,
        id: text.id,
        name: text.name,
        type: "TEXT",
        getStyledTextSegments: () => text.segments.value,
        exportAsync: () => {
            throw Error("Font rasterization attempted");
        },
    } as unknown as SceneNode;
    const results = await generateCodegen(node, Symbol());
    expect(results[0].language).toBe("CSS");
    expect(results[0].code).toContain("StyledText");
    expect(results[0].code).not.toMatch(/@image-url|import/);
});

test("unknown container types cannot export their descendants", async () => {
    const fixture = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    let exports = 0;
    const node = {
        ...fixture.root.properties,
        id: "unknown",
        name: "Unknown",
        type: "SLIDE",
        get children() {
            throw Error("Descendants accessed");
        },
    } as unknown as SceneNode;
    const exporter = async (): Promise<never> => {
        exports++;
        throw Error("Subtree export attempted");
    };
    const captured = await captureSource(
        node,
        Symbol(),
        exporter,
        undefined,
        exporter,
        1,
        false,
        undefined,
        undefined,
        4,
        false,
        undefined,
        undefined,
        false,
        "root-only",
    );
    expect(exports).toBe(0);
    expect(captured.source.root.exports).toBeUndefined();
    expect(captured.source.root.errors).toEqual([]);
});

test("root warnings remain separate from copyable Slint", async () => {
    const fixture = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    const node = {
        ...fixture.root.properties,
        id: "warn",
        name: "Warning",
        type: "RECTANGLE",
        blendMode: "MULTIPLY",
    } as SceneNode;
    const result = await generateCodegen(node, Symbol());
    expect(result[0].language).toBe("CSS");
    expect(result[1]).toMatchObject({
        title: "Diagnostics",
        language: "PLAINTEXT",
    });
    expect(result[1].code).toContain("BLEND");
    expect(result[0].code).not.toContain("BLEND");
});

test.each(["inner-shadow", "asymmetric-rounded-stroke"])(
    "root-only %s preserves generated appearance helpers",
    async (name) => {
        const source = JSON.parse(
            await readFile(`fixtures/source/${name}.json`, "utf8"),
        );
        const normalized = await normalizeSource(source, "export");
        expect(normalized.ok).toBe(true);
        if (!normalized.ok || normalized.empty) return;
        const result = convertSnapshot(normalized.snapshot, {
            scope: "root-only",
        });
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(
            result.source.match(/(?:Rectangle|Image|Path) \{/g)?.length,
        ).toBeGreaterThan(1);
        expect(result.source).not.toContain("export component");
    },
);
