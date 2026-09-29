// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { afterEach, expect, test, vi } from "vitest";
import {
    codegenAliases,
    codegenVariablePath,
    type CodegenVariable,
} from "../src/plugin/codegen-variables";
import { captureCodegenVariables } from "../src/plugin/capture";
import { convertSnapshot } from "../src/preview/converter";
import { generateCodegen } from "../src/plugin/codegen";

afterEach(() => vi.unstubAllGlobals());
const variable = (field: string, type = "FLOAT"): CodegenVariable => ({
    field,
    type,
    name: "Spacing/Small",
    collection: "Design Tokens",
    modes: 2,
});

test("preserves published collection, hierarchy and mode naming", () => {
    expect(codegenVariablePath(variable("width"))).toBe(
        "design-tokens.current.spacing.small",
    );
    expect(
        codegenVariablePath({
            ...variable("width"),
            modes: 1,
            name: ".Palette/2 Blue & Green",
        }),
    ).toBe("design-tokens.palette._2-Blue-and-Green");
    expect(
        codegenVariablePath({
            ...variable("width"),
            collection: "Bad; injected",
        }),
    ).toBeUndefined();
});

test("root variables replace native bindings without changing tree export", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    );
    snapshot.root = snapshot.root.children[0];
    snapshot.root.fills = [
        { kind: "solid", color: { r: 1, g: 0, b: 0, a: 1 }, opacity: 1 },
    ];
    const before = JSON.stringify(snapshot);
    const literal = convertSnapshot(snapshot, { scope: "root-only" });
    const result = convertSnapshot(snapshot, {
        scope: "root-only",
        codegenVariables: [
            variable("width"),
            variable("height"),
            variable("fills", "COLOR"),
        ],
    });
    expect(result.ok && result.source).toContain(
        "width: design-tokens.current.spacing.small;",
    );
    expect(result.ok && result.source).toContain(
        "height: design-tokens.current.spacing.small;",
    );
    expect(result.ok && result.source).toContain(
        "background: design-tokens.current.spacing.small;",
    );
    expect(literal.ok && literal.source).not.toContain("design-tokens");
    expect(JSON.stringify(snapshot)).toBe(before);
    expect(
        convertSnapshot(snapshot, { codegenVariables: [variable("width")] }),
    ).toEqual(convertSnapshot(snapshot));
});

test("text bindings preserve string, numeric and paint variable references", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    );
    snapshot.root = snapshot.root.children.find(
        (node: { kind: string }) => node.kind === "text",
    );
    expect(snapshot.root).toBeDefined();
    const result = convertSnapshot(snapshot, {
        scope: "root-only",
        codegenVariables: [
            variable("characters", "STRING"),
            variable("fontSize"),
            variable("fontWeight"),
            variable("fontFamily", "STRING"),
            variable("fills", "COLOR"),
        ],
    });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    for (const name of [
        "text",
        "font-size",
        "font-weight",
        "font-family",
        "color",
    ])
        expect(result.source).toContain(
            `${name}: design-tokens.current.spacing.small;`,
        );
});

test("missing, invalid and unsupported bindings keep resolved literals with diagnostics", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    );
    const result = convertSnapshot(snapshot, {
        scope: "root-only",
        codegenVariables: [
            { field: "width" },
            variable("height", "STRING"),
            variable("unsupported"),
        ],
    });
    const literal = convertSnapshot(snapshot, { scope: "root-only" });
    expect(result.ok && result.source).toEqual(literal.ok && literal.source);
    expect(
        result.ok &&
            result.warnings.filter(
                (w) => w.code === "CODEGEN_VARIABLE_FALLBACK",
            ),
    ).toHaveLength(3);
});

test("captures only referenced root variables, deduplicates lookups and contains host failures", async () => {
    const source = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    source.root.properties.boundVariables = {
        width: { type: "VARIABLE_ALIAS", id: "size" },
        height: { type: "VARIABLE_ALIAS", id: "size" },
        opacity: { type: "VARIABLE_ALIAS", id: "missing" },
    };
    const lookup = vi.fn(async (id: string) => {
        if (id === "missing") throw Error("removed");
        return {
            name: "Spacing/Small",
            variableCollectionId: "tokens",
            resolvedType: "FLOAT",
        };
    });
    vi.stubGlobal("figma", {
        variables: {
            getVariableByIdAsync: lookup,
            getVariableCollectionByIdAsync: async () => ({
                name: "Design Tokens",
                modes: [{}, {}],
            }),
        },
    });
    expect(codegenAliases(source.root)).toHaveLength(3);
    expect(await captureCodegenVariables(source.root)).toEqual([
        variable("width"),
        variable("height"),
        { field: "opacity" },
    ]);
    expect(lookup).toHaveBeenCalledTimes(2);
});

test("native preference controls variable capture without opening the preview", async () => {
    const source = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    const node = {
        ...source.root.properties,
        id: "root",
        name: "Root",
        type: "FRAME",
        boundVariables: { width: { type: "VARIABLE_ALIAS", id: "width" } },
    } as unknown as SceneNode;
    const lookup = vi.fn(async () => ({
        name: "Spacing/Small",
        variableCollectionId: "tokens",
        resolvedType: "FLOAT",
    }));
    vi.stubGlobal("figma", {
        variables: {
            getVariableByIdAsync: lookup,
            getVariableCollectionByIdAsync: async () => ({
                name: "Design Tokens",
                modes: [{}, {}],
            }),
        },
    });
    const literal = await generateCodegen(node, Symbol(), false);
    expect(lookup).not.toHaveBeenCalled();
    const bound = await generateCodegen(node, Symbol(), true);
    expect(bound[0].code).toContain(
        "width: design-tokens.current.spacing.small;",
    );
    expect(literal[0].code).not.toContain("design-tokens");
});

test("default-valued native properties retain their variable bindings", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/button.snapshot.json", "utf8"),
    );
    snapshot.root = snapshot.root.children[0];
    snapshot.root.fills = [];
    snapshot.root.strokes = [];
    snapshot.root.cornerRadii = [0, 0, 0, 0];
    const literal = convertSnapshot(snapshot, { scope: "root-only" });
    expect(literal.ok && literal.source).not.toContain("border-radius:");
    expect(literal.ok && literal.source).not.toContain("opacity:");
    const result = convertSnapshot(snapshot, {
        scope: "root-only",
        codegenVariables: [variable("opacity"), variable("cornerRadius")],
    });
    expect(result.ok && result.source).toContain(
        "opacity: design-tokens.current.spacing.small;",
    );
    expect(result.ok && result.source).toContain(
        "border-radius: design-tokens.current.spacing.small;",
    );
});
