// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { describe, expect, test } from "vitest";
import { normalizeSource } from "../src/plugin/normalize";
import { captureSelectionSource, captureSource } from "../src/plugin/capture";
import { packCaptureAssets, unpackCaptureAssets } from "../src/asset-transport";
import {
    decodeValue,
    SOURCE_MIXED,
    type SourceCapture,
    type SourceNode,
} from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";

describe("generator", () => {
    async function fixture() {
        return JSON.parse(
            await readFile("fixtures/source/component-variants.json", "utf8"),
        ) as SourceCapture;
    }
    async function convert(source: SourceCapture) {
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Normalization failed");
        const result = convertSnapshot(normalized.snapshot);
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        return { normalized, result };
    }

    test("capture follows main components and captures an off-tree base definition once", async () => {
        const source = await fixture();
        // One fixture-backed API boundary test; conversion tests below use only JSON.
        const nodes = new Map<string, Record<string, unknown>>();
        const hydrate = (
            node: SourceNode,
            parent: Record<string, unknown> | null = null,
        ): Record<string, unknown> => {
            const existing = nodes.get(node.id);
            if (existing) return existing;
            const live: Record<string, unknown> = {
                ...decodeValue(node.properties),
                id: node.id,
                name: node.name,
                type: node.type,
                parent,
            };
            nodes.set(node.id, live);
            if (node.children)
                live.children = node.children.map((child) =>
                    hydrate(child, live),
                );
            return live;
        };
        const root = hydrate(source.root);
        for (const definition of source.components?.definitions ?? []) {
            for (const variant of definition.variants)
                hydrate(variant.root).variantProperties = variant.values;
            const owner = nodes.get(definition.id);
            if (owner)
                owner.componentPropertyDefinitions = {
                    ...definition.contract?.properties,
                    ...Object.fromEntries(
                        Object.entries(definition.axes).map(([key, axis]) => [
                            key,
                            {
                                type: "VARIANT",
                                defaultValue: axis.defaultValue,
                                variantOptions: axis.options,
                            },
                        ]),
                    ),
                };
        }
        for (const [id, live] of nodes) {
            if (live.type === "COMPONENT")
                live.componentPropertyDefinitions ??= {};
            if (live.type === "INSTANCE")
                live.getMainComponentAsync = async () =>
                    nodes.get(
                        source.components?.references[id].variantId ?? "",
                    ) ?? null;
        }
        const captured = await captureSource(
            root as unknown as SceneNode,
            SOURCE_MIXED,
            async () => "",
            async () => undefined,
        );
        expect(
            captured.source.components?.definitions.map((d) => d.id).sort(),
        ).toEqual(["base:1", "button:set"]);
        expect(
            captured.source.components?.definitions.find(
                (d) => d.id === "button:set",
            )?.variants,
        ).toHaveLength(30);
        expect(captured.source.components?.references["layout:button"]).toEqual(
            {
                definitionId: "button:set",
                variantId: "button:1",
            },
        );
        expect((await convert(captured.source)).result.source).toContain(
            '"Save changes"',
        );
    });

    test("emits one family definition, dependencies before consumers, and exact sparse variant selectors", async () => {
        const source = await fixture();
        const { result } = await convert(source);
        expect(
            result.source.match(/export component Button inherits/g),
        ).toHaveLength(1);
        expect(result.source).not.toContain("export component ButtonBase");
        expect(
            result.source.indexOf("export component Button inherits"),
        ).toBeLessThan(result.source.indexOf("export component Demo"));
        expect(result.source).toContain(
            "export enum ButtonState { busy, disabled, enabled, focused, hovered, pressed }",
        );
        expect(result.source).toContain("root.variant-state");
        expect(result.source).not.toMatch(
            /selected-variant|value-\d|Axis\d|FigmaButton/,
        );
        expect(result.source).toContain("valid-variant");
        expect(result.source).toContain('"Save changes"');
        expect(result.source).not.toContain("structure-override");
        expect(result.source).toContain(
            'in property <string> label: "Button";',
        );
        expect(result.source).toContain("variant-state: ButtonState.enabled");
        expect((await convert(source)).result.source).toBe(result.source);
        source.components?.definitions.reverse();
        for (const definition of source.components?.definitions ?? [])
            definition.variants.reverse();
        expect((await convert(source)).result.source).toBe(result.source);
    });

    test("component graph survives binary capture transport and does not mutate its input", async () => {
        const source = await fixture();
        const before = JSON.stringify(source);
        const packed = packCaptureAssets(source);
        expect(
            unpackCaptureAssets(packed.captureJson, packed.captureAssets),
        ).toEqual(source);
        await convert(source);
        expect(JSON.stringify(source)).toBe(before);
        const dependency = source.components?.definitions[1].variants[0].root;
        if (!dependency) throw Error("Missing dependency");
        dependency.exports = {
            svg: {},
            png: {
                value: Array.from(
                    await readFile("fixtures/authored/square.png"),
                ),
            },
        };
        const withAsset = packCaptureAssets(source);
        expect(withAsset.captureAssets).toHaveLength(1);
        expect(
            unpackCaptureAssets(withAsset.captureJson, withAsset.captureAssets),
        ).toEqual(source);
    });

    test("invalid references, duplicate tuples and versions fail validation", async () => {
        for (const mutation of [
            (source: SourceCapture) => {
                if (source.components) source.components.version = 2 as 1;
            },
            (source: SourceCapture) => {
                if (source.components)
                    source.components.references["layout:button"].variantId =
                        "missing";
            },
            (source: SourceCapture) => {
                const variants = source.components?.definitions[0].variants;
                if (variants) variants[1].values = variants[0].values;
            },
        ]) {
            const source = await fixture();
            mutation(source);
            await expect(normalizeSource(source)).rejects.toThrow(
                "component library",
            );
        }
    });

    test("cyclic component dependencies report an error instead of emitting forward references", async () => {
        const { normalized } = await convert(await fixture());
        const library = normalized.snapshot.components;
        if (!library) throw Error("Missing library");
        const base = library.definitions.find((d) => d.id === "base:1");
        if (!base || !("children" in base.variants[0].root))
            throw Error("Missing base");
        library.references[base.variants[0].root.children[0].id] = {
            definitionId: "button:set",
            variantId: "button:1",
        };
        expect(convertSnapshot(normalized.snapshot)).toMatchObject({
            ok: false,
            diagnostics: [{ code: "COMPONENT_GENERATION_ERROR" }],
        });
    });

    test("failed component lookup returns diagnostics instead of rejecting capture", async () => {
        const node = {
            id: "instance:broken",
            name: "Broken",
            type: "INSTANCE",
            getMainComponentAsync: async () => {
                throw new Error("Library unavailable");
            },
        } as unknown as SceneNode;
        const result = await captureSelectionSource(
            [node],
            SOURCE_MIXED,
            async () => "",
            async () => undefined,
        );
        expect(result.ok).toBe(false);
        if (!result.ok)
            expect(result.diagnostics).toEqual([
                expect.objectContaining({
                    code: "CAPTURE_ERROR",
                    message: "Library unavailable",
                }),
            ]);
    });

    test("names only disambiguate actual collisions and reserve component names before enums", async () => {
        const source = await fixture();
        const library = source.components!;
        const button = library.definitions.find((d) => d.id === "button:set")!;
        const base = library.definitions.find((d) => d.id === "base:1")!;
        base.name = "ButtonState";
        let result = (await convert(source)).result.source;
        expect(result).not.toContain("export component ButtonState inherits");
        expect(result).toContain("export enum ButtonState2 {");
        base.name = "Button";
        result = (await convert(source)).result.source;
        expect(result).not.toContain("export component Button inherits");
        expect(result).toContain("export component Button2 inherits");
        library.definitions.reverse();
        expect((await convert(source)).result.source).toBe(result);
        button.name = "Rectangle";
        result = (await convert(source)).result.source;
        expect(result).toContain(
            "export component Rectangle2 inherits Rectangle",
        );
    });

    function family(source: string) {
        return source
            .slice(
                0,
                source.indexOf("\ncomponent ButtonOverride") >= 0
                    ? source.indexOf("\ncomponent ButtonOverride")
                    : source.indexOf("export component Demo"),
            )
            .trimEnd();
    }

    test("authored API and family body do not depend on selected instance overrides", async () => {
        const source = await fixture();
        const baseline = family((await convert(source)).result.source);
        source.root.children = source.root.children?.filter(
            (n) => n.id !== "layout:button",
        );
        const without = (await convert(source)).result.source;
        expect(family(without)).toBe(baseline);
        expect(without).toContain('in property <string> label: "Button"');
        const instance = structuredClone(source.root.children![0]);
        const rename = (node: SourceNode) => {
            node.id = `custom:${node.id}`;
            for (const child of node.children ?? []) rename(child);
        };
        rename(instance);
        instance.properties.opacity = 0.25;
        source.root.children!.push(instance);
        source.components!.references[instance.id] =
            source.components!.references[source.root.children![0].id];
        const changed = (await convert(source)).result;
        expect(family(changed.source)).toBe(baseline);
        expect(changed.source).toContain("component ButtonOverride inherits");
        expect(
            changed.warnings.some(
                (w) => w.code === "COMPONENT_INSTANCE_SPECIALIZED",
            ),
        ).toBe(true);
    });

    test("component output has a bounded API and factors the gallery below flattened size", async () => {
        const { normalized, result } = await convert(await fixture());
        const flat = convertSnapshot({
            ...normalized.snapshot,
            components: undefined,
        });
        if (!flat.ok) throw Error("Flattened conversion failed");
        expect(result.source.length).toBeLessThan(flat.source.length);
        expect(
            Math.max(...result.source.split("\n").map((line) => line.length)),
        ).toBeLessThanOrEqual(120);
        expect(result.source.match(/in property/g)).toHaveLength(4);
        expect(result.source).not.toMatch(
            /structure-override|selected-structure|text-color-2/,
        );
        expect(result.source.match(/Text \{/g)).toHaveLength(1);
    });

    test("malformed authored properties cannot cross the capture contract", async () => {
        const source = await fixture();
        source.components!.definitions[0].contract!.properties[
            "Label#1"
        ].defaultValue = false;
        await expect(normalizeSource(source)).rejects.toThrow(
            "component library",
        );
    });

    test("numeric animation axes stay integers with an explicit allowed domain", async () => {
        const source = await fixture();
        const d = source.components!.definitions[0];
        const options = d.axes.State.options;
        const numbers = Object.fromEntries(
            options.map((value, i) => [value, String(i + 1)]),
        );
        d.axes.Step = {
            defaultValue: numbers.Enabled,
            options: Object.values(numbers),
        };
        delete d.axes.State;
        for (const v of d.variants) {
            v.values.Step = numbers[v.values.State];
            delete v.values.State;
        }
        const result = (await convert(source)).result.source;
        expect(result).toContain("in property <int> variant-step:");
        expect(result).toContain("// Step: allowed values 1, 2, 3, 4, 5, 6.");
        expect(result).not.toContain("option-");
        expect(result).not.toContain("enum ButtonStep");
    });

    test("variable aliases survive normalization and invalid cycles are diagnosed", async () => {
        const source = JSON.parse(
            await readFile("fixtures/source/component-tokens.json", "utf8"),
        ) as SourceCapture;
        const { normalized, result } = await convert(source);
        expect(normalized.snapshot.variables).toEqual(source.variables);
        expect(result.source).toContain("export global DesignTokens");
        source.variables!.variables.palette.values.light = {
            type: "VARIABLE_ALIAS",
            id: "button",
        };
        const failed = await normalizeSource(source);
        if (!failed.ok || failed.empty) throw Error("Normalization failed");
        expect(convertSnapshot(failed.snapshot)).toMatchObject({
            ok: false,
            diagnostics: [
                {
                    code: "COMPONENT_GENERATION_ERROR",
                    message: expect.stringContaining("Cyclic variable alias"),
                },
            ],
        });
    });

    test("root opacity and authored visibility retain the sparse variant guard", async () => {
        const source = await fixture();
        const definition = source.components!.definitions[0];
        definition.contract!.properties["Show#2"] = {
            type: "BOOLEAN",
            defaultValue: true,
        };
        for (const variant of definition.variants) {
            variant.root.properties.opacity = 0.5;
            definition.contract!.bindings[variant.root.id] = {
                visible: "Show#2",
            };
        }
        const { result } = await convert(source);
        expect(result.source).toContain(
            "opacity: (root.valid-variant) && (root.show) ? (0.5) : 0;",
        );
    });

    test("state output keeps the public contract and removes private style selectors and default instance arguments", async () => {
        const { result } = await convert(await fixture());
        const [library, demo] = result.source.split("export component Demo");
        expect(library).toContain("states [");
        expect(library).not.toMatch(/private property <(?:brush|length)>/);
        expect(library).not.toContain("? true :");
        expect(demo).not.toContain("variant-state: ButtonState.enabled");
        expect(demo).not.toContain("variant-color: ButtonColor.primary");
        expect(demo).not.toContain("variant-style: ButtonStyle.filled");
        expect(library).toContain("in property <ButtonState>");
    });

    test("identical image payloads are declared once before the component that uses them", async () => {
        const capture = JSON.parse(
            await readFile(
                "fixtures/source/component-raster-icon.json",
                "utf8",
            ),
        ) as SourceCapture;
        const { result } = await convert(capture);
        expect(result.source.match(/@image-url\(/g)).toHaveLength(1);
        expect(result.source).toContain("source: Assets.");
        expect(result.source.indexOf("global Assets")).toBeLessThan(
            result.source.indexOf("export component"),
        );
    });
});

describe("defaults", () => {
    async function fixture(name: string) {
        return JSON.parse(
            await readFile(`fixtures/${name}.snapshot.json`, "utf8"),
        );
    }

    for (const target of ["preview", "export"] as const) {
        const generate = (snapshot: unknown) => {
            const result = convertSnapshot(snapshot, { target });
            if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
            return result.source;
        };

        test(`${target}: reusable component omits constant builtin appearance defaults`, async () => {
            const capture = JSON.parse(
                await readFile(
                    "fixtures/source/component-variants.json",
                    "utf8",
                ),
            );
            const definition = capture.components.definitions.find(
                (item: { id: string }) => item.id === "button:set",
            );
            for (const variant of definition.variants) {
                Object.assign(variant.root.properties, {
                    fills: [],
                    strokes: [],
                    effects: [],
                    opacity: 1,
                    clipsContent: false,
                    cornerRadius: 0,
                    topLeftRadius: 0,
                    topRightRadius: 0,
                    bottomLeftRadius: 0,
                    bottomRightRadius: 0,
                });
                variant.root.children = [];
            }
            capture.root = definition.variants[0].root;
            const normalized = await normalizeSource(capture, target);
            if (!normalized.ok || normalized.empty)
                throw Error(JSON.stringify(normalized));
            const code = generate(normalized.snapshot).split(
                "export component Demo",
            )[0];
            expect(code).toContain("export component Button");
            expect(code).not.toMatch(
                /(?:background|border-color): transparent;/,
            );
            expect(code).not.toMatch(/border-(?:width|radius): 0px;/);
            expect(code).not.toMatch(/(?:clip: false|opacity: 1);/);
        });

        test(`${target}: full-parent rectangle omits zero positions and redundant geometry`, async () => {
            const capture = await fixture("image-fill");
            capture.root.fills = [
                {
                    kind: "solid",
                    color: { r: 1, g: 0, b: 0, a: 1 },
                    opacity: 1,
                },
            ];
            const code = generate(capture);
            expect(code).not.toMatch(/\b[xy]: 0px;/);
            expect(code.match(/\bwidth:/g)).toHaveLength(1);
            expect(code.match(/\bheight:/g)).toHaveLength(1);
        });

        test(`${target}: rounded image wrapper keeps only the position needed to suppress intrinsic sizing`, async () => {
            const capture = await fixture("image-fill");
            capture.root.cornerRadii = [8, 8, 8, 8];
            const code = generate(capture);
            expect(code.match(/\bx: 0px;/g)).toHaveLength(1);
            expect(code).not.toContain("y: 0px;");
            expect(code).toContain("clip: true;");
        });

        test(`${target}: empty fill container does not redundantly reset its zero minimum size`, async () => {
            const capture = await fixture("nested-fill");
            const child = structuredClone(capture.root);
            Object.assign(child, {
                id: "empty",
                children: [],
                autoLayout: null,
                layoutSizingHorizontal: "fill",
                layoutSizingVertical: "fill",
            });
            capture.root.children = [child];
            const code = generate(capture);
            expect(code).not.toContain("min-width: 0px;");
            expect(code).not.toContain("min-height: 0px;");
        });

        test(`${target}: text and nested layouts keep zero minimum overrides needed to shrink`, async () => {
            const code = generate(await fixture("nested-fill"));
            expect(code.match(/min-width: 0px;/g)).toHaveLength(2);
            expect(code.match(/min-height: 0px;/g)).toHaveLength(1);
        });

        test(`${target}: smaller absolute children retain zero positions instead of becoming centered`, async () => {
            const capture = await fixture("nested-fill");
            capture.root.autoLayout = null;
            const child = structuredClone(capture.root);
            Object.assign(child, {
                id: "small",
                x: 0,
                y: 0,
                width: 20,
                height: 20,
                children: [],
                layoutSizingHorizontal: "fixed",
                layoutSizingVertical: "fixed",
            });
            capture.root.children = [child];
            const code = generate(capture);
            expect(code).toContain("x: 0px;");
            expect(code).toContain("y: 0px;");
            expect(code).toContain("width: 20px;");
        });
    }
});
