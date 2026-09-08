// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { describe, expect, test } from "vitest";
import {
    type CaptureInstrumentation,
    captureSelection,
    captureSource,
} from "../src/plugin/capture";
import { parseSnapshot } from "../src/plugin/snapshot";

const solid = (r: number, g: number, b: number): SolidPaint => ({
    type: "SOLID",
    color: { r, g, b },
    opacity: 1,
});

test("validates real malformed snapshot fixtures at the JSON boundary", async () => {
    const fixtures = [
        ["mixed-text-style.json", "INVALID_SNAPSHOT"],
        ["unsupported-node.json", "INVALID_SNAPSHOT"],
        ["gradient-stop-count.json", "INVALID_SNAPSHOT"],
    ] as const;
    for (const [name, code] of fixtures) {
        const result = parseSnapshot(
            await readFile(`fixtures/errors/${name}`, "utf8"),
        );
        expect(result.ok, name).toBe(false);
        if (result.ok) continue;
        expect(result.diagnostics, name).toContainEqual(
            expect.objectContaining({ code }),
        );
    }
});

test("accepts the v8 auto-layout fixture and rejects previous schema versions", async () => {
    const json = await readFile("fixtures/auto-layout.snapshot.json", "utf8");
    const parsed = parseSnapshot(json);
    expect(parsed.ok).toBe(true);
    const oldVersion = JSON.parse(json) as { schemaVersion: number };
    for (const schemaVersion of [1, 2, 3, 4, 5, 6, 7]) {
        oldVersion.schemaVersion = schemaVersion;
        expect(parseSnapshot(JSON.stringify(oldVersion))).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ code: "INVALID_SNAPSHOT" }),
            ],
        });
    }
});

test("requires fields introduced by the strict v8 snapshot contract", async () => {
    const canonical = JSON.parse(
        await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
    ) as { root: Record<string, unknown> };
    for (const property of [
        "layoutPositioning",
        "cornerRadii",
        "clipsContent",
        "shadows",
        "containerKind",
        "layoutFallback",
    ]) {
        const root = structuredClone(canonical.root);
        delete root[property];
        const result = parseSnapshot(JSON.stringify({ ...canonical, root }));
        expect(result.ok, property).toBe(false);
        if (result.ok) continue;
        expect(result.diagnostics, property).toContainEqual(
            expect.objectContaining({ propertyPath: `root.${property}` }),
        );
    }
    const root = structuredClone(canonical.root) as Record<string, unknown> & {
        children?: Record<string, unknown>[];
    };
    const canonicalText = root.children?.find(
        (child) => (child as Record<string, unknown>).kind === "text",
    ) as Record<string, unknown> | undefined;
    if (canonicalText !== undefined) {
        for (const value of [undefined, "invalid"]) {
            const invalidTextRoot = structuredClone(root);
            const invalidTextNode = invalidTextRoot.children?.find(
                (child) => (child as Record<string, unknown>).kind === "text",
            ) as Record<string, unknown>;
            if (value === undefined) invalidTextNode.textAutoResize = undefined;
            else invalidTextNode.textAutoResize = value;
            const result = parseSnapshot(
                JSON.stringify({ ...canonical, root: invalidTextRoot }),
            );
            expect(result.ok, String(value)).toBe(false);
            if (result.ok) continue;
            expect(result.diagnostics, String(value)).toContainEqual(
                expect.objectContaining({
                    propertyPath: "root.children.0.textAutoResize",
                }),
            );
        }
    }
    for (const property of [
        "wrap",
        "counterAxisSpacing",
        "counterAxisAlignContent",
    ]) {
        const missingFieldRoot = structuredClone(root);
        delete (missingFieldRoot.autoLayout as Record<string, unknown>)[
            property
        ];
        const result = parseSnapshot(
            JSON.stringify({ ...canonical, root: missingFieldRoot }),
        );
        expect(result.ok, property).toBe(false);
        if (result.ok) continue;
        expect(result.diagnostics, property).toContainEqual(
            expect.objectContaining({
                propertyPath: `root.autoLayout.${property}`,
            }),
        );
    }
});

test("uses selection fixtures for empty and multiple-selection outcomes", async () => {
    const empty = JSON.parse(
        await readFile("fixtures/errors/empty-selection.json", "utf8"),
    ) as { selection: string[]; expectedOutcome: string };
    const multiple = JSON.parse(
        await readFile("fixtures/errors/multiple-selection.json", "utf8"),
    ) as { selection: string[]; expectedDiagnostics: { code: string }[] };
    expect(await captureSelection([], Symbol())).toMatchObject({
        ok: true,
        empty: true,
        nodeIds: [],
    });
    expect(empty.expectedOutcome).toBe("clear");
    expect(
        await captureSelection(
            [group as unknown as SceneNode, group as unknown as SceneNode],
            Symbol(),
        ),
    ).toMatchObject({
        ok: false,
        diagnostics: multiple.expectedDiagnostics,
    });
    expect(empty.selection).toEqual([]);
    expect(multiple.selection).toHaveLength(2);
});

test("approximates rotated, layered, and mixed text styles while warning on stroke alignment", async () => {
    const rotated = { ...rectangle, rotation: 15 };
    expect(
        await captureSelection([rotated as unknown as SceneNode], Symbol()),
    ).toMatchObject({ ok: true });
    expect(
        await captureSelection(
            [
                {
                    ...rectangle,
                    fills: [solid(1, 0, 0), solid(0, 0, 1)],
                } as unknown as SceneNode,
            ],
            Symbol(),
        ),
    ).toMatchObject({
        ok: true,
        warnings: [{ code: "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED" }],
    });
    expect(
        await captureSelection(
            [{ ...text, fontSize: Symbol("mixed") } as unknown as SceneNode],
            Symbol(),
        ),
    ).toMatchObject({
        ok: true,
        warnings: [{ code: "MIXED_TEXT_STYLE_APPROXIMATED" }],
    });
    const strokeResult = await captureSelection(
        [{ ...rectangle, strokeAlign: "OUTSIDE" } as unknown as SceneNode],
        Symbol(),
    );
    expect(strokeResult).toMatchObject({
        ok: true,
        warnings: [],
        snapshot: { root: { strokes: [{ align: "outside" }] } },
    });
});

const rectangle = {
    type: "RECTANGLE",
    id: "1:2",
    name: "Background",
    x: 0,
    y: 0,
    width: 240,
    height: 72,
    opacity: 1,
    visible: true,
    rotation: 0,
    fills: [solid(0.2, 0.3, 0.4)],
    strokes: [solid(0, 0, 0)],
    strokeWeight: 1,
    strokeAlign: "INSIDE",
    cornerRadius: 12,
};

const text = {
    type: "TEXT",
    id: "1:3",
    name: "Slint",
    x: 0,
    y: 0,
    width: 240,
    height: 72,
    opacity: 1,
    visible: true,
    rotation: 0,
    characters: "Slint",
    fills: [solid(1, 1, 1)],
    fontName: { family: "Inter", style: "Regular" },
    fontSize: 20,
    fontWeight: 400,
    textAlignHorizontal: "CENTER",
    textAlignVertical: "CENTER",
    textAutoResize: "NONE",
};

const group = {
    type: "GROUP",
    id: "1:1",
    name: "Button",
    x: 0,
    y: 0,
    width: 240,
    height: 72,
    opacity: 1,
    visible: true,
    rotation: 0,
    children: [rectangle, text],
};

test("captures only JSON-safe values for the supported button shape", async () => {
    const result = await captureSelection(
        [group as unknown as SceneNode],
        Symbol("figma.mixed"),
    );
    expect(result.ok).toBe(true);
    if (!result.ok || result.empty) return;
    expect(JSON.parse(JSON.stringify(result.snapshot))).toEqual(
        result.snapshot,
    );
    expect(result.nodeIds).toEqual(["1:1", "1:2", "1:3"]);
});

test("instruments only text-bearing SVG exports, including failed exports", async () => {
    const instrumentedTypes: string[] = [];
    const instrumentation: CaptureInstrumentation = {
        measureFontToImageConversion: async (operation) => {
            instrumentedTypes.push("font-to-image");
            return operation();
        },
    };
    const svgNode = (type: string, id: string) =>
        ({
            type,
            id,
            name: id,
            x: 0,
            y: 0,
            width: 40,
            height: 20,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: [],
            strokes: [],
            strokeWeight: 0,
            strokeAlign: "INSIDE",
            cornerRadius: 0,
        }) as unknown as SceneNode;
    const exporter = async (
        node: VectorNode | BooleanOperationNode | TextNode,
    ) => `<svg viewBox="0 0 40 20"><path data-node="${node.id}" /></svg>`;

    for (const [type, id] of [
        ["CONNECTOR", "connector:instrumented"],
        ["SHAPE_WITH_TEXT", "shape:instrumented"],
        ["VECTOR", "vector:uninstrumented"],
    ]) {
        const result = await captureSelection(
            [svgNode(type, id)],
            Symbol("figma.mixed"),
            exporter,
            undefined,
            instrumentation,
        );
        expect(result.ok, type).toBe(true);
    }
    const textResult = await captureSelection(
        [{ ...text, id: "text:uninstrumented" } as unknown as SceneNode],
        Symbol("figma.mixed"),
        exporter,
        undefined,
        instrumentation,
    );
    expect(textResult.ok).toBe(true);
    expect(instrumentedTypes).toEqual(["font-to-image", "font-to-image"]);

    const failedResult = await captureSelection(
        [svgNode("CONNECTOR", "connector:failed")],
        Symbol("figma.mixed"),
        async () => {
            throw new Error("expected export failure");
        },
        undefined,
        instrumentation,
    );
    expect(failedResult.ok).toBe(false);
    expect(instrumentedTypes).toHaveLength(3);
});

function strictObject<T extends object>(value: T): T {
    return new Proxy(value, {
        get(target, property, receiver) {
            if (
                typeof property === "string" &&
                !Object.prototype.hasOwnProperty.call(target, property)
            ) {
                throw new Error(`undeclared Figma read: ${property}`);
            }
            return Reflect.get(target, property, receiver);
        },
    });
}

test("captures a styled Frame through the shared appearance subset", async () => {
    const gradient = strictObject({
        type: "GRADIENT_LINEAR",
        visible: true,
        opacity: 1,
        gradientTransform: [
            [1, 0, 0],
            [0, 1, 0],
        ],
        gradientStops: [
            strictObject({
                position: 0,
                color: strictObject({ r: 0.1, g: 0.4, b: 0.9 }),
            }),
            strictObject({
                position: 1,
                color: strictObject({ r: 0.6, g: 0.2, b: 0.8 }),
            }),
        ],
    });
    const stroke = strictObject({
        type: "SOLID",
        visible: true,
        color: strictObject({ r: 0.1, g: 0.2, b: 0.6 }),
        opacity: 1,
    });
    const child = strictObject({
        type: "TEXT",
        id: "2:2",
        name: "Frame label",
        x: 16,
        y: 16,
        width: 208,
        height: 40,
        opacity: 0.88,
        visible: true,
        rotation: 0,
        characters: "Frame",
        fills: [
            strictObject({
                type: "SOLID",
                visible: true,
                color: strictObject({ r: 1, g: 1, b: 1 }),
                opacity: 1,
            }),
        ],
        fontName: strictObject({ family: "Inter", style: "Regular" }),
        fontSize: 20,
        fontWeight: 400,
        textAlignHorizontal: "CENTER",
        textAlignVertical: "CENTER",
    });
    const frame = strictObject({
        type: "FRAME",
        id: "2:1",
        name: "Styled Frame",
        x: 18,
        y: 24,
        width: 240,
        height: 72,
        opacity: 0.92,
        visible: true,
        rotation: 0,
        layoutMode: "NONE",
        fills: [gradient],
        strokes: [stroke],
        strokeWeight: 2,
        strokeAlign: "INSIDE",
        cornerRadius: 12,
        children: [child],
    });

    const result = await captureSelection(
        [frame as unknown as SceneNode],
        Symbol("figma.mixed"),
    );
    expect(result.ok).toBe(true);
    if (!result.ok || result.empty) return;
    expect(result.snapshot.schemaVersion).toBe(8);
    expect(result.snapshot.root).toMatchObject({
        kind: "frame",
        fills: [
            {
                kind: "linear-gradient",
                stops: [{ position: 0 }, { position: 1 }],
            },
        ],
        strokes: [{ strokeTopWeight: 2, paint: { kind: "solid" } }],
        cornerRadii: [12, 12, 12, 12],
    });
    expect(result.snapshot.root.kind).toBe("frame");
    if (result.snapshot.root.kind !== "frame") return;
    expect(result.snapshot.root.children.map((node) => node.id)).toEqual([
        "2:2",
    ]);
    expect(JSON.parse(JSON.stringify(result.snapshot))).toEqual(
        result.snapshot,
    );
});

test("captures arbitrary gradient stops and image paints into a self-contained snapshot", async () => {
    const gradient = {
        type: "GRADIENT_LINEAR",
        visible: true,
        opacity: 0.8,
        gradientTransform: [
            [1, 0, 0],
            [0, 1, 0],
        ],
        gradientStops: [
            { position: 0, color: { r: 1, g: 0, b: 0 } },
            { position: 0.33, color: { r: 1, g: 1, b: 0 } },
            { position: 0.66, color: { r: 0, g: 1, b: 1 } },
            { position: 1, color: { r: 0, g: 0, b: 1 } },
        ],
    } as unknown as Paint;
    const image = {
        type: "IMAGE",
        visible: true,
        opacity: 1,
        imageHash: "hash",
        scaleMode: "FIT",
    } as unknown as Paint;
    const node = strictObject({
        ...rectangle,
        fills: [gradient, image],
    });
    const result = await captureSelection(
        [node as unknown as SceneNode],
        Symbol("mixed"),
        undefined,
        async () => ({
            bytes: new Uint8Array([
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
            ]),
            width: 16,
            height: 8,
        }),
    );
    expect(result).toMatchObject({
        ok: true,
        warnings: [{ code: "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED" }],
    });

    const gradientNode = strictObject({ ...rectangle, fills: [gradient] });
    const gradientResult = await captureSelection(
        [gradientNode as unknown as SceneNode],
        Symbol("mixed"),
    );
    expect(gradientResult).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                fills: [{ kind: "linear-gradient", stops: [{}, {}, {}, {}] }],
            },
        },
    });

    const imageNode = strictObject({ ...rectangle, fills: [image] });
    const imageResult = await captureSelection(
        [imageNode as unknown as SceneNode],
        Symbol("mixed"),
        undefined,
        async () => ({
            bytes: new Uint8Array([
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
            ]),
            width: 16,
            height: 8,
        }),
    );
    expect(imageResult).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                fills: [
                    {
                        kind: "image",
                        mimeType: "image/png",
                        intrinsicWidth: 16,
                        intrinsicHeight: 8,
                    },
                ],
            },
        },
    });
});

test("resolves each image hash once per capture", async () => {
    const image = {
        type: "IMAGE",
        visible: true,
        opacity: 1,
        imageHash: "shared-hash",
        scaleMode: "FILL",
    } as unknown as Paint;
    const children = ["1:4", "1:5"].map((id) =>
        strictObject({
            ...rectangle,
            id,
            fills: [image],
            strokes: [],
        }),
    );
    let resolutions = 0;
    const result = await captureSelection(
        [{ ...group, children } as unknown as SceneNode],
        Symbol("mixed"),
        undefined,
        async () => {
            resolutions += 1;
            return {
                bytes: new Uint8Array([
                    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
                ]),
                width: 16,
                height: 8,
            };
        },
    );
    expect(result.ok).toBe(true);
    expect(resolutions).toBe(1);
});

function autoLayoutFrame(
    updates: Record<string, unknown> = {},
    childUpdates: Record<string, unknown> = {},
): SceneNode {
    const autoChild = strictObject({
        ...text,
        id: "3:2",
        name: "Auto child",
        layoutPositioning: "AUTO",
        layoutSizingHorizontal: "HUG",
        layoutSizingVertical: "FIXED",
        ...childUpdates,
    });
    return strictObject({
        type: "FRAME",
        id: "3:1",
        name: "Auto Layout Button",
        x: 0,
        y: 0,
        width: 320,
        height: 96,
        opacity: 1,
        visible: true,
        rotation: 0,
        fills: [solid(0.12, 0.32, 0.84)],
        strokes: [],
        strokeWeight: 0,
        strokeAlign: "INSIDE",
        cornerRadius: 14,
        layoutMode: "HORIZONTAL",
        layoutWrap: "NO_WRAP",
        itemReverseZIndex: false,
        strokesIncludedInLayout: false,
        paddingLeft: 16,
        paddingRight: 16,
        paddingTop: 12,
        paddingBottom: 12,
        itemSpacing: 12,
        primaryAxisAlignItems: "CENTER",
        counterAxisAlignItems: "CENTER",
        layoutSizingHorizontal: "FIXED",
        layoutSizingVertical: "FIXED",
        children: [autoChild],
        ...updates,
    }) as unknown as SceneNode;
}

function componentLikeNode(
    type: "COMPONENT" | "INSTANCE",
    id: string,
    children: readonly object[],
    updates: Record<string, unknown> = {},
): SceneNode {
    return strictObject({
        type,
        id,
        name: `${type} button`,
        x: 0,
        y: 0,
        width: 240,
        height: 72,
        opacity: 1,
        visible: true,
        rotation: 0,
        layoutPositioning: "AUTO",
        layoutSizingHorizontal: "FIXED",
        layoutSizingVertical: "FIXED",
        layoutMode: "HORIZONTAL",
        layoutWrap: "NO_WRAP",
        itemReverseZIndex: false,
        strokesIncludedInLayout: false,
        paddingLeft: 16,
        paddingRight: 16,
        paddingTop: 12,
        paddingBottom: 12,
        itemSpacing: 12,
        primaryAxisAlignItems: "CENTER",
        counterAxisAlignItems: "CENTER",
        fills: [solid(0.12, 0.32, 0.84)],
        strokes: [],
        strokeWeight: 0,
        strokeAlign: "INSIDE",
        cornerRadius: 14,
        children,
        ...(type === "INSTANCE" ? { scaleFactor: 1 } : {}),
        ...updates,
    }) as unknown as SceneNode;
}

function componentText(
    id: string,
    characters: string,
    updates: Record<string, unknown> = {},
): object {
    return strictObject({
        ...text,
        id,
        name: "Resolved label",
        layoutPositioning: "AUTO",
        layoutSizingHorizontal: "HUG",
        layoutSizingVertical: "FIXED",
        characters,
        ...updates,
    });
}

test("captures Components and resolved Instance children as styled containers", async () => {
    const componentChild = componentText("4:2", "Component");
    const component = componentLikeNode("COMPONENT", "4:1", [componentChild]);
    const instanceChild = componentText("5:2", "Override");
    const instance = componentLikeNode("INSTANCE", "5:1", [instanceChild]);

    const componentResult = await captureSelection(
        [component],
        Symbol("mixed"),
    );
    const instanceResult = await captureSelection([instance], Symbol("mixed"));
    expect(componentResult.ok).toBe(true);
    expect(instanceResult.ok).toBe(true);
    if (
        !componentResult.ok ||
        componentResult.empty ||
        !instanceResult.ok ||
        instanceResult.empty
    )
        return;
    expect(componentResult.snapshot.root.kind).toBe("component");
    expect(instanceResult.snapshot.root).toMatchObject({
        kind: "instance",
        children: [{ kind: "text", characters: "Override" }],
    });
    expect(instanceResult.nodeIds).toEqual(["5:1", "5:2"]);
});

test("captures only the referenced private component variant", async () => {
    const childReads = new Map<string, number>();
    const variant = (id: string) => {
        const value = {
            type: "COMPONENT",
            id,
            name: id,
            visible: true,
            componentPropertyDefinitions: {},
            variantProperties: { State: id },
        } as unknown as ComponentNode;
        childReads.set(id, 0);
        Object.defineProperty(value, "children", {
            get: () => {
                childReads.set(id, (childReads.get(id) ?? 0) + 1);
                return [];
            },
        });
        return value;
    };
    const variants = Array.from({ length: 132 }, (_, index) =>
        variant(`icon:${index}`),
    );
    const owner = {
        type: "COMPONENT_SET",
        id: "icon:set",
        name: "Icon",
        children: variants,
        componentPropertyDefinitions: {},
    } as unknown as ComponentSetNode;
    for (const child of variants)
        Object.defineProperty(child, "parent", { value: owner });
    const instance = {
        type: "INSTANCE",
        id: "icon:use",
        name: "Icon use",
        visible: true,
        children: [],
        componentProperties: {},
        getMainComponentAsync: async () => variants[1],
    } as unknown as InstanceNode;
    const result = await captureSource(
        instance,
        Symbol("mixed"),
        async () => "<svg />",
        undefined,
        undefined,
        1,
        false,
        undefined,
        undefined,
        1,
        true,
    );
    expect(result.source.components?.definitions).toHaveLength(1);
    expect(result.source.components?.definitions[0].scope).toBe("private");
    expect(
        result.source.components?.definitions[0].variants.map((v) => v.id),
    ).toEqual(["icon:1"]);
    expect(childReads.get("icon:1")).toBe(1);
    expect([...childReads.values()].filter((count) => count > 0)).toHaveLength(
        1,
    );
    expect(result.work).toMatchObject({
        capturedNodes: 2,
        componentFamilies: 1,
        componentVariants: 1,
        pngExports: 0,
        svgExports: 0,
    });

    const readsBeforePublic = new Map(childReads);
    const publicResult = await captureSource(
        owner as unknown as SceneNode,
        Symbol("mixed"),
        async () => "<svg />",
        undefined,
        undefined,
        1,
        false,
        undefined,
        undefined,
        1,
        true,
    );
    expect(publicResult.source.components?.definitions[0].scope).toBe(
        "complete",
    );
    expect(
        publicResult.source.components?.definitions[0].variants,
    ).toHaveLength(132);
    expect(publicResult.work).toMatchObject({
        capturedNodes: 133,
        componentFamilies: 1,
        componentVariants: 132,
    });
    for (const variant of variants)
        expect(
            childReads.get(variant.id)! - readsBeforePublic.get(variant.id)!,
        ).toBe(1);

    const selectedVariantResult = await captureSource(
        variants[0] as unknown as SceneNode,
        Symbol("mixed"),
        async () => "<svg />",
        undefined,
        undefined,
        1,
        false,
        undefined,
        undefined,
        1,
        true,
    );
    expect(selectedVariantResult.source.components?.definitions[0].scope).toBe(
        "complete",
    );
    expect(
        selectedVariantResult.source.components?.definitions[0].variants,
    ).toHaveLength(132);
});

test("rejects a component reference whose required variant is unavailable", async () => {
    const available = {
        type: "COMPONENT",
        id: "available",
        name: "Available",
        visible: true,
        children: [],
        componentPropertyDefinitions: {},
    } as unknown as ComponentNode;
    const owner = {
        type: "COMPONENT_SET",
        id: "owner",
        name: "Owner",
        children: [available],
        componentPropertyDefinitions: {},
    } as unknown as ComponentSetNode;
    Object.defineProperty(available, "parent", { value: owner });
    const missing = {
        type: "COMPONENT",
        id: "missing",
        name: "Missing",
        parent: owner,
    } as unknown as ComponentNode;
    const instance = {
        type: "INSTANCE",
        id: "invalid-use",
        name: "Invalid use",
        visible: true,
        children: [],
        componentProperties: {},
        getMainComponentAsync: async () => missing,
    } as unknown as InstanceNode;
    await expect(
        captureSource(
            instance,
            Symbol("mixed"),
            async () => "<svg />",
            undefined,
            undefined,
            1,
            false,
            undefined,
            undefined,
            1,
            true,
        ),
    ).rejects.toThrow(/Component reachability failed/);
});

test("retains hidden children bound to a component visibility property", async () => {
    const hidden = {
        type: "RECTANGLE",
        id: "visibility:hidden",
        name: "Conditional content",
        visible: false,
        componentPropertyReferences: { visible: "Show content" },
        children: [],
    } as unknown as SceneNode;
    const component = {
        type: "COMPONENT",
        id: "visibility:component",
        name: "Visibility component",
        visible: true,
        children: [hidden],
        componentPropertyDefinitions: {
            "Show content": { type: "BOOLEAN", defaultValue: false },
        },
    } as unknown as SceneNode;
    const result = await captureSource(
        component,
        Symbol("mixed"),
        async () => "<svg />",
        undefined,
        undefined,
        1,
        false,
        undefined,
        undefined,
        1,
        true,
    );
    const captured = result.source.components?.definitions[0].variants[0].root;
    expect(captured?.children?.map((child) => child.id)).toContain(
        "visibility:hidden",
    );
});

test("captures nested Instances and reports stable component diagnostics", async () => {
    const nestedChild = componentText("6:3", "Nested override");
    const nested = componentLikeNode("INSTANCE", "6:2", [nestedChild], {
        width: 144,
        height: 48,
        scaleFactor: 1,
    });
    const root = componentLikeNode("INSTANCE", "6:1", [nested], {
        scaleFactor: 1,
    });
    const nestedResult = await captureSelection([root], Symbol("mixed"));
    expect(nestedResult.ok).toBe(true);
    if (!nestedResult.ok || nestedResult.empty) return;
    expect(nestedResult.snapshot.root).toMatchObject({
        kind: "instance",
        children: [
            {
                kind: "instance",
                children: [{ kind: "text", characters: "Nested override" }],
            },
        ],
    });
    expect(nestedResult.nodeIds).toEqual(["6:1", "6:2", "6:3"]);

    const scaled = await captureSelection(
        [componentLikeNode("INSTANCE", "7:1", [], { scaleFactor: 2 })],
        Symbol("mixed"),
    );
    expect(scaled).toMatchObject({ ok: true, warnings: [] });
    const variants = [
        componentLikeNode("COMPONENT", "7:3", [
            componentText("7:4", "Default"),
        ]),
        componentLikeNode(
            "COMPONENT",
            "7:5",
            [componentText("7:6", "Pressed")],
            { x: 160, y: 80 },
        ),
    ];
    const componentSet = strictObject({
        type: "COMPONENT_SET",
        id: "7:2",
        name: "Variants",
        x: 0,
        y: 0,
        width: 320,
        height: 160,
        opacity: 1,
        visible: true,
        rotation: 0,
        layoutPositioning: "AUTO",
        layoutSizingHorizontal: "FIXED",
        layoutSizingVertical: "FIXED",
        layoutMode: "NONE",
        fills: [],
        strokes: [],
        strokeWeight: 0,
        strokeAlign: "INSIDE",
        cornerRadius: 0,
        clipsContent: false,
        children: variants,
    });
    expect(
        await captureSelection(
            [componentSet as unknown as SceneNode],
            Symbol("mixed"),
        ),
    ).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "component-set",
                children: [
                    { kind: "component", id: "7:3", x: 0, y: 0 },
                    { kind: "component", id: "7:5", x: 160, y: 80 },
                ],
            },
        },
        nodeIds: ["7:2", "7:3", "7:4", "7:5", "7:6"],
    });
});

test("captures Vector and Boolean-operation nodes as complete injected SVG leaves", async () => {
    const vector = {
        type: "VECTOR",
        id: "8:1",
        name: "Vector icon",
        x: 0,
        y: 0,
        width: 24,
        height: 24,
        opacity: 1,
        visible: true,
        rotation: 0,
    } as unknown as SceneNode;
    const booleanOperation = {
        type: "BOOLEAN_OPERATION",
        id: "8:2",
        name: "Boolean icon",
        x: 0,
        y: 0,
        width: 24,
        height: 24,
        opacity: 1,
        visible: true,
        rotation: 0,
        children: [
            {
                type: "VECTOR",
                id: "8:3",
                name: "Boolean path",
            },
        ],
    } as unknown as SceneNode;
    const settings: string[] = [];
    const exporter = async (
        node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        settings.push(node.type);
        return `<svg viewBox="0 0 24 24"><path data-node="${node.id}" /></svg>`;
    };

    const vectorResult = await captureSelection(
        [vector],
        Symbol("mixed"),
        exporter,
    );
    const booleanResult = await captureSelection(
        [booleanOperation],
        Symbol("mixed"),
        exporter,
    );
    expect(vectorResult).toMatchObject({
        ok: true,
        snapshot: { root: { kind: "svg", sourceType: "VECTOR" } },
    });
    expect(booleanResult).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "svg",
                sourceType: "BOOLEAN_OPERATION",
                svg: expect.stringContaining('data-node="8:2"'),
            },
        },
        nodeIds: ["8:2", "8:3"],
    });
    expect(settings).toEqual(["VECTOR", "BOOLEAN_OPERATION"]);
});

test("captures basic Figma shape nodes through the same SVG boundary", async () => {
    const types = ["ELLIPSE", "LINE", "POLYGON", "STAR"] as const;
    const exporter = async (
        node: VectorNode | BooleanOperationNode | TextNode,
    ) => `<svg viewBox="0 0 24 24"><path data-node="${node.id}" /></svg>`;
    for (const [index, type] of types.entries()) {
        const shape = {
            type,
            id: `8:${index + 10}`,
            name: `${type} shape`,
            x: 0,
            y: 0,
            width: 24,
            height: 24,
            opacity: 1,
            visible: true,
            rotation: 0,
        } as unknown as SceneNode;
        const result = await captureSelection(
            [shape],
            Symbol("mixed"),
            exporter,
        );
        expect(result, type).toMatchObject({
            ok: true,
            snapshot: { root: { kind: "svg", sourceType: type } },
        });
    }
});

test("captures text typography and truncation metadata", async () => {
    const styledText = {
        ...text,
        fontName: { family: "Inter", style: "Italic" },
        letterSpacing: { value: 10, unit: "PERCENT" },
        lineHeight: { value: 24, unit: "PIXELS" },
        textAutoResize: "HEIGHT",
        textTruncation: "ENDING",
        maxLines: 2,
    } as unknown as SceneNode;
    const result = await captureSelection([styledText], Symbol("mixed"));
    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "text",
                italic: true,
                letterSpacing: 2,
                lineHeightFactor: 1.2,
                textAutoResize: "height",
                wrap: true,
                overflow: "elide",
                maxLines: 2,
            },
        },
    });
});

test("captures conservative icon-font text as outlined SVG leaves", async () => {
    const exportedTypes: string[] = [];
    const exporter = async (
        node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportedTypes.push(node.type);
        return '<svg viewBox="0 0 24 24"><path d="M0 0h24v24H0z" /></svg>';
    };
    const icon = {
        ...text,
        id: "8:5",
        name: "Cloud icon",
        characters: ` ${String.fromCodePoint(0xe8b6)} `,
        fontName: { family: "Inter", style: "Regular" },
    } as unknown as SceneNode;
    const supplementary = {
        ...icon,
        id: "8:6",
        name: "Supplementary icon",
        characters: String.fromCodePoint(0xf0000),
    } as unknown as SceneNode;
    const ligature = {
        ...icon,
        id: "8:7",
        name: "Ligature icon",
        characters: "cloud_upload",
        fontName: { family: "Material Symbols Rounded", style: "Regular" },
    } as unknown as SceneNode;

    for (const node of [icon, supplementary, ligature]) {
        const result = await captureSelection(
            [node],
            Symbol("mixed"),
            exporter,
        );
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: "svg",
                    sourceType: "TEXT",
                    x: 0,
                    y: 0,
                    width: 240,
                    height: 72,
                },
            },
        });
    }
    expect(exportedTypes).toEqual(["TEXT", "TEXT", "TEXT"]);
});

test.each([false, true])(
    "skips SVG for PNG capture, including overflowing paint: %s",
    async (overflow) => {
        const pngBytes = new Uint8Array(
            await readFile("fixtures/authored/square.png"),
        );
        let svgCalls = 0;
        let pngCalls = 0;
        const icon = {
            ...text,
            ...(overflow
                ? {
                      type: "VECTOR",
                      absoluteTransform: [
                          [1, 0, 100],
                          [0, 1, 100],
                      ],
                      absoluteBoundingBox: {
                          x: 100,
                          y: 100,
                          width: 24,
                          height: 24,
                      },
                      absoluteRenderBounds: {
                          x: 95,
                          y: 97,
                          width: 34,
                          height: 34,
                      },
                  }
                : {}),
            id: "8:raster-icon",
            name: "Raster-backed icon",
            characters: String.fromCodePoint(0xe8b6),
        } as unknown as SceneNode;
        const result = await captureSelection(
            [icon],
            Symbol("mixed"),
            async () => {
                svgCalls += 1;
                return '<svg width="23" height="17" viewBox="0 0 23 17"><path /></svg>';
            },
            undefined,
            undefined,
            async () => {
                pngCalls += 1;
                return pngBytes;
            },
        );

        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: "svg",
                    sourceType: overflow ? "VECTOR" : "TEXT",
                    raster: {
                        exportScale: 1,
                        pixelWidth: 24,
                        pixelHeight: 24,
                    },
                },
            },
        });
        expect(svgCalls).toBe(0);
        expect(pngCalls).toBe(1);
    },
);

test("captures Material Symbols ligatures when node-level weight is mixed", async () => {
    const mixedValue = Symbol("figma.mixed");
    const icon = {
        ...text,
        id: "8:7-mixed-weight",
        name: "cloud_upload",
        width: 24,
        height: 24,
        characters: "cloud_upload",
        fontName: { family: "Material Symbols Outlined", style: "Medium" },
        fontSize: 24,
        fontWeight: mixedValue,
        getStyledTextSegments: () => [
            {
                characters: "cloud_upload",
                start: 0,
                end: "cloud_upload".length,
                fontName: {
                    family: "Material Symbols Outlined",
                    style: "Medium",
                },
                fontSize: 24,
                fontWeight: 500,
            },
        ],
    } as unknown as SceneNode;
    const result = await captureSelection(
        [icon],
        mixedValue,
        async () =>
            '<svg width="23" height="17" viewBox="0 0 23 17"><path /></svg>',
    );

    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "svg",
                sourceType: "TEXT",
            },
        },
        captureMetrics: { requests: 1, exports: 1, cacheHits: 0 },
    });
    if (!result.ok || result.empty) return;
    expect(result.snapshot.root.kind).toBe("svg");
    if (result.snapshot.root.kind !== "svg") return;
    expect(result.snapshot.root.svg).toBe(
        '<svg width="24" height="24" viewBox="-0.5 -3.5 24 24"><path /></svg>',
    );
});

test("restores node-sized SVG viewports for vector and text exports", async () => {
    const exporter = async () =>
        '<svg width="23" height="17" viewBox="0 0 23 17"><path d="M1 2h20v3H1z" /></svg>';
    const vector = {
        type: "VECTOR",
        id: "8:viewport-vector",
        name: "Cropped vector",
        x: 0,
        y: 0,
        width: 24,
        height: 24,
        opacity: 1,
        visible: true,
        rotation: 0,
    } as unknown as SceneNode;
    const textIcon = {
        ...text,
        id: "8:viewport-text",
        name: "Cropped text icon",
        width: 24,
        height: 24,
        characters: "cloud_upload",
        fontName: { family: "Material Symbols Rounded", style: "Regular" },
    } as unknown as SceneNode;
    for (const node of [vector, textIcon]) {
        const result = await captureSelection(
            [node],
            Symbol("mixed"),
            exporter,
        );
        expect(result.ok, node.id).toBe(true);
        if (!result.ok || result.empty) continue;
        expect(result.snapshot.root.kind, node.id).toBe("svg");
        if (result.snapshot.root.kind !== "svg") continue;
        expect(result.snapshot.root.svg, node.id).toBe(
            '<svg width="24" height="24" viewBox="-0.5 -3.5 24 24"><path d="M1 2h20v3H1z" /></svg>',
        );
    }
});

test("keeps ordinary, mixed-style, and multiline text on the text path", async () => {
    const exported: string[] = [];
    let ordinarySegmentReads = 0;
    const exporter = async (
        node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exported.push(node.id);
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const ordinary = {
        ...text,
        id: "8:8",
        characters: "ordinary label",
        getStyledTextSegments: () => {
            ordinarySegmentReads += 1;
            return [
                {
                    characters: "ordinary label",
                    start: 0,
                    end: "ordinary label".length,
                    fontName: { family: "Inter", style: "Regular" },
                    fontSize: 20,
                    fontWeight: 400,
                    fills: [solid(1, 1, 1)],
                },
            ];
        },
    } as unknown as SceneNode;
    const mixed = {
        ...text,
        id: "8:9",
        characters: String.fromCodePoint(0xe8b6),
        getStyledTextSegments: () => [
            {
                characters: String.fromCodePoint(0xe8b6),
                fontName: { family: "Inter", style: "Regular" },
                fontSize: 20,
                fontWeight: 400,
            },
            {
                characters: "x",
                fontName: { family: "Inter", style: "Bold" },
                fontSize: 20,
                fontWeight: 700,
            },
        ],
    } as unknown as SceneNode;
    const multiline = {
        ...text,
        id: "8:10",
        characters: `${String.fromCodePoint(0xe8b6)}\n${String.fromCodePoint(0xe8b7)}`,
    } as unknown as SceneNode;

    const mixedIconAndOrdinary = {
        ...text,
        id: "8:12",
        characters: "cloud_uploadlabel",
        fontName: Symbol("mixed-font"),
        getStyledTextSegments: () => [
            {
                characters: "cloud_upload",
                start: 0,
                end: "cloud_upload".length,
                fontName: {
                    family: "Material Symbols Outlined",
                    style: "Medium",
                },
                fontSize: 20,
                fontWeight: 500,
            },
            {
                characters: "label",
                start: "cloud_upload".length,
                end: "cloud_uploadlabel".length,
                fontName: { family: "Inter", style: "Regular" },
                fontSize: 20,
                fontWeight: 400,
            },
        ],
    } as unknown as SceneNode;

    for (const node of [ordinary, mixed, multiline]) {
        const result = await captureSelection(
            [node],
            Symbol("mixed"),
            exporter,
        );
        expect(result).toMatchObject({
            ok: true,
            snapshot: { root: { kind: "text" } },
        });
    }
    expect(exported).toEqual([]);
    expect(ordinarySegmentReads).toBe(1);
    const mixedResult = await captureSelection(
        [mixedIconAndOrdinary],
        Symbol("mixed"),
        exporter,
    );
    expect(mixedResult).toMatchObject({
        ok: true,
        snapshot: { root: { kind: "svg" } },
    });
    expect(exported).toEqual([mixedIconAndOrdinary.id]);
});

test("rejects icon-font SVG output that is malformed or still contains text", async () => {
    const icon = {
        ...text,
        id: "8:11",
        characters: String.fromCodePoint(0xe8b6),
    } as unknown as SceneNode;
    const malformed = await captureSelection(
        [icon],
        Symbol("mixed"),
        async () => "<path />",
    );
    expect(malformed).toMatchObject({
        ok: false,
        diagnostics: [{ code: "TEXT_SVG_EXPORT_INVALID" }],
    });
    const unoutlined = await captureSelection(
        [icon],
        Symbol("mixed"),
        async () => '<svg viewBox="0 0 24 24"><text>cloud</text></svg>',
    );
    expect(unoutlined).toMatchObject({
        ok: false,
        diagnostics: [{ code: "TEXT_SVG_EXPORT_INVALID" }],
    });
    const rejected = await captureSelection(
        [icon],
        Symbol("mixed"),
        async () => {
            throw new Error("font export rejected");
        },
    );
    expect(rejected).toMatchObject({
        ok: false,
        diagnostics: [{ code: "SVG_EXPORT_FAILED", nodeId: "8:11" }],
    });
});

test("does not call a failing SVG exporter when PNG succeeds", async () => {
    const icon = {
        ...text,
        id: "8:11-png-fallback",
        characters: String.fromCodePoint(0xe8b6),
    } as unknown as SceneNode;
    const result = await captureSelection(
        [icon],
        Symbol("mixed"),
        async () => {
            throw new Error("This node may not have any visible layers");
        },
        undefined,
        undefined,
        async () =>
            new Uint8Array(await readFile("fixtures/authored/square.png")),
    );

    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "svg",
                sourceType: "TEXT",
                raster: {
                    exportScale: 1,
                    pixelWidth: 24,
                    pixelHeight: 24,
                },
            },
        },
        warnings: [],
    });
    if (!result.ok || result.empty) return;
    expect(result.snapshot.root.kind).toBe("svg");
    if (result.snapshot.root.kind !== "svg") return;
    expect(result.snapshot.root.svg).toBeUndefined();
});

test.each(["rejected", "empty", "invalid-header"])(
    "reports a %s PNG export without attempting SVG",
    async (failure) => {
        const calls: string[] = [];
        const vector = {
            ...rectangle,
            type: "VECTOR",
            id: "8:svg-fallback",
            name: "SVG-backed icon",
            width: 24,
            height: 24,
        } as unknown as SceneNode;
        const result = await captureSelection(
            [vector],
            Symbol("mixed"),
            async () => {
                calls.push("svg");
                return '<svg viewBox="0 0 24 24"><path /></svg>';
            },
            undefined,
            undefined,
            async () => {
                calls.push("png");
                if (failure === "rejected")
                    throw new Error("PNG export unavailable");
                return new Uint8Array(failure === "empty" ? [] : [1, 2, 3]);
            },
        );

        expect(calls).toEqual(["png"]);
        expect(result).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ code: "PNG_EXPORT_FAILED" }),
            ],
        });
    },
);

test("skips invisible descendants before attempting visual exports", async () => {
    let exportCalls = 0;
    const hiddenIcon = {
        ...text,
        id: "8:12",
        name: "Hidden cloud icon",
        visible: false,
        characters: String.fromCodePoint(0xe8b6),
    } as unknown as SceneNode;
    const hiddenVector = {
        ...rectangle,
        type: "VECTOR",
        id: "8:13",
        name: "Hidden vector",
        visible: false,
    } as unknown as SceneNode;
    const hiddenNestedIcon = {
        ...hiddenIcon,
        id: "8:16",
        name: "Hidden nested cloud icon",
    } as unknown as SceneNode;
    const hiddenGroup = {
        ...group,
        id: "8:14",
        name: "Hidden group",
        visible: false,
        children: [hiddenNestedIcon],
    } as unknown as SceneNode;
    const root = {
        ...group,
        id: "8:15",
        name: "Component set with hidden layers",
        children: [hiddenIcon, hiddenVector, hiddenGroup],
    } as unknown as SceneNode;

    const result = await captureSelection([root], Symbol("mixed"), async () => {
        exportCalls += 1;
        throw new Error("hidden node should never reach exportAsync");
    });

    expect(result).toMatchObject({
        ok: true,
        snapshot: { root: { children: [] } },
        captureMetrics: { requests: 0, exports: 0, cacheHits: 0 },
    });
    expect(result.nodeIds).toEqual(["8:15", "8:12", "8:13", "8:14", "8:16"]);
    expect(exportCalls).toBe(0);
});

test("exports every icon node independently within one capture", async () => {
    let exportCalls = 0;
    const exporter = async (
        _node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportCalls += 1;
        await Promise.resolve();
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const icons = Array.from(
        { length: 10 },
        (_, index) =>
            ({
                ...text,
                id: `8:${20 + index}`,
                name: `Repeated icon ${index}`,
                x: index * 32,
                characters: String.fromCodePoint(0xe8b6),
                getStyledTextSegments: () => [
                    {
                        characters: String.fromCodePoint(0xe8b6),
                        start: 0,
                        end: 1,
                        fontName: { family: "Inter", style: "Regular" },
                        fontSize: 20,
                        fontWeight: 400,
                    },
                ],
            }) as unknown as SceneNode,
    );
    const root = {
        ...group,
        id: "8:30",
        name: "Repeated icons",
        children: icons,
    } as unknown as SceneNode;
    const result = await captureSelection([root], Symbol("mixed"), exporter);

    expect(result).toMatchObject({
        ok: true,
        captureMetrics: {
            requests: 10,
            exports: 10,
            cacheHits: 0,
        },
    });
    if (!result.ok || result.empty) return;
    expect(result.captureMetrics.durationMs).toBeGreaterThanOrEqual(0);
    expect(result.captureMetrics.requests).toBe(
        result.captureMetrics.exports + result.captureMetrics.cacheHits,
    );
    expect(exportCalls).toBe(10);
});

test("does not reuse an icon raster that can contain another node's background", async () => {
    const icons = [
        {
            ...text,
            id: "8:60",
            name: "Icon with first raster",
            characters: String.fromCodePoint(0xe8b6),
        },
        {
            ...text,
            id: "8:61",
            name: "Icon with second raster",
            characters: String.fromCodePoint(0xe8b6),
        },
    ] as unknown as SceneNode[];
    const root = {
        ...group,
        id: "8:62",
        name: "Independent icon rasters",
        children: icons,
    } as unknown as SceneNode;
    let svgExports = 0;
    let pngExports = 0;
    const result = await captureSelection(
        [root],
        Symbol("mixed"),
        async () => {
            svgExports += 1;
            return '<svg viewBox="0 0 24 24"><path /></svg>';
        },
        undefined,
        undefined,
        async () => {
            pngExports += 1;
            return new Uint8Array(
                await readFile(
                    pngExports === 1
                        ? "fixtures/authored/square.png"
                        : "fixtures/authored/asymmetric.png",
                ),
            );
        },
    );

    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                children: [
                    { raster: { pixelWidth: 24, pixelHeight: 24 } },
                    { raster: { pixelWidth: 32, pixelHeight: 24 } },
                ],
            },
        },
        captureMetrics: { requests: 2, exports: 2, cacheHits: 0 },
    });
    expect(svgExports).toBe(0);
    expect(pngExports).toBe(2);
});

test("exports icons when styled segments are unavailable without cross-node deduplication", async () => {
    let exportCalls = 0;
    const exporter = async (
        _node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportCalls += 1;
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const icon = (id: string) =>
        ({
            ...text,
            id,
            characters: "cloud_upload",
            fontName: {
                family: "Material Symbols Outlined",
                style: "Medium",
            },
            getStyledTextSegments: () => {
                throw new Error("font data unavailable");
            },
        }) as unknown as SceneNode;
    const root = {
        ...group,
        id: "8:31",
        children: [icon("8:32"), icon("8:33")],
    } as unknown as SceneNode;

    const result = await captureSelection([root], Symbol("mixed"), exporter);

    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                children: [{ kind: "svg" }, { kind: "svg" }],
            },
        },
        captureMetrics: { requests: 2, exports: 2, cacheHits: 0 },
    });
    expect(exportCalls).toBe(2);
});

test("keeps icon export signatures distinct for visual and variable-mode changes", async () => {
    let exportCalls = 0;
    const exporter = async (
        _node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportCalls += 1;
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const icon = (overrides: Record<string, unknown>) =>
        ({
            ...text,
            characters: String.fromCodePoint(0xe8b6),
            ...overrides,
        }) as unknown as SceneNode;
    const variants = [
        icon({ id: "8:40", name: "same id differs", x: 10 }),
        icon({
            id: "8:41",
            name: "different glyph",
            characters: String.fromCodePoint(0xe8b7),
        }),
        icon({
            id: "8:42",
            name: "different font",
            fontName: { family: "Font Awesome 6 Free", style: "Regular" },
        }),
        icon({ id: "8:43", name: "different size", fontSize: 24 }),
        icon({ id: "8:44", name: "different fill", fills: [solid(0, 0, 0)] }),
        icon({ id: "8:45", name: "different bounds", width: 48 }),
        icon({
            id: "8:46",
            name: "different variable mode",
            resolvedVariableModes: { "collection:1": "mode:2" },
        }),
    ];
    const root = {
        ...group,
        id: "8:47",
        name: "Distinct icons",
        children: variants,
    } as unknown as SceneNode;
    const result = await captureSelection([root], Symbol("mixed"), exporter);

    expect(result).toMatchObject({
        ok: true,
        captureMetrics: { requests: 7, exports: 7, cacheHits: 0 },
    });
    expect(exportCalls).toBe(7);
});

test("does not deduplicate icons with different per-run fills", async () => {
    let exportCalls = 0;
    const exporter = async (
        _node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportCalls += 1;
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const glyph = String.fromCodePoint(0xe8b6);
    const iconWithRunFill = (id: string, x: number, fill: SolidPaint) =>
        ({
            ...text,
            id,
            x,
            characters: glyph,
            getStyledTextSegments: () => [
                {
                    characters: glyph,
                    start: 0,
                    end: glyph.length,
                    fontName: { family: "Inter", style: "Regular" },
                    fontSize: 20,
                    fontWeight: 400,
                    fills: [fill],
                },
            ],
        }) as unknown as SceneNode;
    const icons = [
        iconWithRunFill("8:50", 0, solid(1, 1, 1)),
        iconWithRunFill("8:51", 32, solid(0, 0, 0)),
    ];
    const root = {
        ...group,
        id: "8:52",
        children: icons,
    } as unknown as SceneNode;

    const result = await captureSelection([root], Symbol("mixed"), exporter);

    expect(result).toMatchObject({
        ok: true,
        captureMetrics: { requests: 2, exports: 2, cacheHits: 0 },
    });
    expect(exportCalls).toBe(2);
});

test("does not retain icon exports between captures and counts failed attempts", async () => {
    const icon = {
        ...text,
        id: "8:50",
        characters: String.fromCodePoint(0xe8b6),
    } as unknown as SceneNode;
    let exportCalls = 0;
    const exporter = async (
        _node: VectorNode | BooleanOperationNode | TextNode,
    ) => {
        exportCalls += 1;
        return '<svg viewBox="0 0 24 24"><path /></svg>';
    };
    const first = await captureSelection([icon], Symbol("mixed"), exporter);
    const second = await captureSelection([icon], Symbol("mixed"), exporter);
    expect(first).toMatchObject({
        ok: true,
        captureMetrics: { requests: 1, exports: 1, cacheHits: 0 },
    });
    expect(second).toMatchObject({
        ok: true,
        captureMetrics: { requests: 1, exports: 1, cacheHits: 0 },
    });
    expect(exportCalls).toBe(2);

    const failed = await captureSelection(
        [icon],
        Symbol("mixed"),
        async (_node: VectorNode | BooleanOperationNode | TextNode) => {
            throw new Error("temporary export failure");
        },
    );
    expect(failed).toMatchObject({
        ok: false,
        captureMetrics: { requests: 1, exports: 1, cacheHits: 0 },
    });
    if (failed.ok) return;
    expect(failed.captureMetrics.durationMs).toBeGreaterThanOrEqual(0);
});

test("normalizes every Figma text auto-resize mode and mixed values", async () => {
    const values = [
        ["NONE", "none"],
        ["WIDTH_AND_HEIGHT", "width-and-height"],
        ["HEIGHT", "height"],
        ["TRUNCATE", "truncate"],
    ] as const;
    for (const [value, expected] of values) {
        const result = await captureSelection(
            [
                {
                    ...text,
                    id: `resize:${value}`,
                    textAutoResize: value,
                } as unknown as SceneNode,
            ],
            Symbol("mixed"),
        );
        expect(result, value).toMatchObject({
            ok: true,
            snapshot: { root: { textAutoResize: expected } },
        });
    }
    const mixedValue = Symbol("figma.mixed");
    const mixed = await captureSelection(
        [{ ...text, textAutoResize: mixedValue } as unknown as SceneNode],
        mixedValue,
    );
    expect(mixed).toMatchObject({
        ok: true,
        snapshot: { root: { textAutoResize: "none" } },
        warnings: [
            expect.objectContaining({
                code: "MIXED_TEXT_STYLE_APPROXIMATED",
                propertyPath: "textAutoResize",
            }),
        ],
    });
    const unavailable = { ...text } as Record<string, unknown>;
    unavailable.textAutoResize = undefined;
    const unavailableResult = await captureSelection(
        [unavailable as unknown as SceneNode],
        Symbol("mixed"),
    );
    expect(unavailableResult).toMatchObject({
        ok: true,
        snapshot: { root: { textAutoResize: "none" } },
        warnings: [
            expect.objectContaining({
                code: "MIXED_TEXT_STYLE_APPROXIMATED",
                propertyPath: "textAutoResize",
            }),
        ],
    });
});

test("turns SVG export rejection and malformed output into stable diagnostics", async () => {
    const vector = {
        type: "VECTOR",
        id: "8:4",
        name: "Broken icon",
        x: 0,
        y: 0,
        width: 24,
        height: 24,
        opacity: 1,
        visible: true,
        rotation: 0,
    } as unknown as SceneNode;
    const rejected = await captureSelection(
        [vector],
        Symbol("mixed"),
        async () => {
            throw new Error("removed during export");
        },
    );
    expect(rejected).toMatchObject({
        ok: false,
        diagnostics: [
            {
                code: "SVG_EXPORT_FAILED",
                nodeId: "8:4",
                nodeName: "Broken icon",
            },
        ],
    });
    const malformed = await captureSelection(
        [vector],
        Symbol("mixed"),
        async () => "<path />",
    );
    expect(malformed).toMatchObject({
        ok: false,
        diagnostics: [{ code: "SVG_EXPORT_FAILED" }],
    });
});

test("captures fixed horizontal auto-layout metadata and direct-child sizing", async () => {
    const result = await captureSelection([autoLayoutFrame()], Symbol("mixed"));
    expect(result.ok).toBe(true);
    if (!result.ok || result.empty) return;
    expect(result.snapshot.schemaVersion).toBe(8);
    expect(result.snapshot.root.kind).toBe("frame");
    if (result.snapshot.root.kind !== "frame") return;
    expect(result.snapshot.root.autoLayout).toEqual({
        direction: "horizontal",
        paddingLeft: 16,
        paddingRight: 16,
        paddingTop: 12,
        paddingBottom: 12,
        itemSpacing: 12,
        wrap: false,
        counterAxisSpacing: 12,
        counterAxisAlignContent: "start",
        primaryAlignment: "center",
        counterAlignment: "center",
    });
    expect(result.snapshot.root.layoutSizingHorizontal).toBe("fixed");
    expect(result.snapshot.root.children[0]).toMatchObject({
        layoutSizingHorizontal: "hug",
        layoutSizingVertical: "fixed",
    });
    expect(result.nodeIds).toEqual(["3:1", "3:2"]);
});

test("captures wrapped auto-layout gaps and line distribution", async () => {
    const result = await captureSelection(
        [
            autoLayoutFrame({
                layoutWrap: "WRAP",
                counterAxisSpacing: 20,
                counterAxisAlignContent: "SPACE_BETWEEN",
            }),
        ],
        Symbol("mixed"),
    );
    expect(result).toMatchObject({
        ok: true,
        snapshot: {
            root: {
                kind: "frame",
                autoLayout: {
                    wrap: true,
                    counterAxisSpacing: 20,
                    counterAxisAlignContent: "space-between",
                },
            },
        },
    });
});

test("warns for unsupported layout properties while retaining invalid geometry errors", async () => {
    const cases: [string, Record<string, unknown>, Record<string, unknown>][] =
        [
            ["GRID_LAYOUT_APPROXIMATED", { layoutMode: "GRID" }, {}],
            [
                "STROKES_IN_LAYOUT_APPROXIMATED",
                { strokesIncludedInLayout: true },
                {},
            ],
            ["LAYOUT_PADDING_APPROXIMATED", { paddingLeft: -1 }, {}],
            ["LAYOUT_SPACING_APPROXIMATED", { itemSpacing: Number.NaN }, {}],
            [
                "BASELINE_ALIGNMENT_APPROXIMATED",
                { counterAxisAlignItems: "BASELINE" },
                {},
            ],
            [
                "LAYOUT_SIZING_APPROXIMATED",
                {},
                { layoutSizingHorizontal: "BOGUS" },
            ],
            ["INVALID_GEOMETRY", { width: -1 }, {}],
        ];
    for (const [code, updates, childUpdates] of cases) {
        const result = await captureSelection(
            [autoLayoutFrame(updates, childUpdates)],
            Symbol("mixed"),
        );
        const warningCase = code.endsWith("APPROXIMATED");
        expect(result.ok, code).toBe(warningCase);
        if (warningCase) {
            if (!result.ok || result.empty) continue;
            expect(result.warnings, code).toContainEqual(
                expect.objectContaining({ code }),
            );
        } else {
            if (result.ok) continue;
            expect(result.diagnostics, code).toContainEqual(
                expect.objectContaining({ code }),
            );
        }
        expect(result.nodeIds, code).toContain("3:1");
    }
});

test("keeps Groups transparent while validating Frame appearance boundaries", async () => {
    const frame = parseSnapshot(
        await readFile("fixtures/frame.snapshot.json", "utf8"),
    );
    expect(frame.ok).toBe(true);
    if (!frame.ok) return;
    expect(frame.snapshot.root.kind).toBe("frame");
    const groupResult = await captureSelection(
        [group as unknown as SceneNode],
        Symbol("figma.mixed"),
    );
    expect(groupResult.ok).toBe(true);
    if (groupResult.ok && !groupResult.empty) {
        expect(groupResult.snapshot.root.kind).toBe("group");
        expect(groupResult.snapshot.root).not.toHaveProperty("fills");
        expect(groupResult.snapshot.root).not.toHaveProperty("strokes");
    }
    const malformed = JSON.parse(JSON.stringify(frame.snapshot)) as {
        root: { fills: unknown[] };
    };
    malformed.root.fills.push(malformed.root.fills[0]);
    expect(parseSnapshot(JSON.stringify(malformed))).toMatchObject({
        ok: true,
    });
});

test("returns stable diagnostics for selection and unsupported-node failures", async () => {
    expect(await captureSelection([], Symbol())).toMatchObject({
        ok: true,
        empty: true,
    });
    expect(
        await captureSelection(
            [group as unknown as SceneNode, group as unknown as SceneNode],
            Symbol(),
        ),
    ).toMatchObject({
        ok: false,
        diagnostics: [{ code: "MULTIPLE_SELECTION" }],
    });
    expect(
        await captureSelection(
            [
                {
                    type: "SLICE",
                    id: "1:4",
                    name: "Circle",
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                    opacity: 1,
                    visible: true,
                    rotation: 0,
                } as unknown as SceneNode,
            ],
            Symbol(),
        ),
    ).toMatchObject({
        ok: true,
        warnings: expect.arrayContaining([
            expect.objectContaining({ code: "NON_VISUAL_NODE_SKIPPED" }),
        ]),
    });
});

test("unavailable fonts always export complete text layers", async () => {
    const mixed = Symbol("mixed");
    for (const [family, style, characters, kind] of [
        ["Roboto", "Regular", "Hello", "svg"],
        ["Roboto", "Regular", "Hello\nWorld", "svg"],
        ["Inter", "Bold Italic", "Hello", "text"],
        [" inter ", "Regular", "Hello", "text"],
        ["Roboto", "Regular", "", "text"],
        ["Material Symbols Outlined", "Regular", "home", "svg"],
    ] as const) {
        let exports = 0;
        const result = await captureSelection(
            [
                {
                    ...text,
                    characters,
                    fontName: { family, style },
                } as unknown as SceneNode,
            ],
            mixed,
            async () => {
                exports++;
                return '<svg width="240" height="72"><path d="M0 0h20v20z"/></svg>';
            },
            undefined,
            undefined,
            undefined,
            2,
        );
        expect(result).toMatchObject({
            ok: true,
            snapshot: { root: { kind } },
        });
        expect(exports).toBe(kind === "svg" ? 1 : 0);
        expect(result.captureMetrics.exports).toBe(exports);
    }
});

test("unavailable font conversion reads mixed fonts once and exports the entire layer", async () => {
    const mixed = Symbol("mixed");
    for (const secondFamily of ["Inter", "Roboto"]) {
        let reads = 0;
        const node = {
            ...text,
            characters: "Hello world",
            fontName: mixed,
            getStyledTextSegments: () => {
                reads++;
                return [
                    {
                        start: 0,
                        end: 6,
                        characters: "Hello ",
                        fontName: { family: "Inter", style: "Bold" },
                    },
                    {
                        start: 6,
                        end: 11,
                        characters: "world",
                        fontName: { family: secondFamily, style: "Regular" },
                    },
                ];
            },
        } as unknown as unknown as SceneNode;
        const result = await captureSelection(
            [node],
            mixed,
            async () =>
                '<svg width="240" height="72"><path d="M0 0h20v20z"/></svg>',
            undefined,
            undefined,
            undefined,
            1,
        );
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: secondFamily === "Inter" ? "text" : "svg",
                    width: 240,
                    height: 72,
                },
            },
        });
        expect(reads).toBe(1);
    }
});

test("unavailable font conversion reports unresolved mixed fonts and invalid image exports", async () => {
    const mixed = Symbol("mixed");
    const unresolved = await captureSelection(
        [
            {
                ...text,
                fontName: mixed,
                getStyledTextSegments: () => {
                    throw new Error("unavailable");
                },
            } as unknown as unknown as SceneNode,
        ],
        mixed,
        undefined,
        undefined,
        undefined,
        undefined,
        1,
    );
    expect(unresolved).toMatchObject({
        ok: false,
        diagnostics: [{ code: "TEXT_FONT_UNRESOLVED" }],
    });
    const invalid = await captureSelection(
        [
            {
                ...text,
                fontName: { family: "Roboto", style: "Regular" },
            } as unknown as SceneNode,
        ],
        mixed,
        async () => "<svg><text>Hello</text></svg>",
        undefined,
        undefined,
        undefined,
        1,
    );
    expect(invalid).toMatchObject({
        ok: false,
        diagnostics: [{ code: "TEXT_SVG_EXPORT_INVALID" }],
    });
});

test("superseded captures stop before exporting or reading remaining siblings", async () => {
    const { captureSelectionSource } = await import("../src/plugin/capture");
    const fixture = JSON.parse(
        await readFile("fixtures/source/negative-gap.json", "utf8"),
    );
    let cancelled = false;
    let release: () => void = () => {};
    let started: () => void = () => {};
    const pending = new Promise<void>((resolve) => {
        release = resolve;
    });
    const exporting = new Promise<void>((resolve) => {
        started = resolve;
    });
    let unscheduledReads = 0;
    const children = [0, 1, 2, 3, 4, 5].map((id) => ({
        ...fixture.root.properties,
        id: `vector:${id}`,
        name: `Vector ${id}`,
        type: "VECTOR",
        get fills() {
            if (id >= 4) unscheduledReads++;
            return [];
        },
    }));
    const root = {
        ...fixture.root.properties,
        id: fixture.root.id,
        name: fixture.root.name,
        type: fixture.root.type,
        children,
    } as unknown as SceneNode;
    const exports: string[] = [];
    const capture = captureSelectionSource(
        [root],
        Symbol("mixed"),
        async (node) => {
            exports.push(node.id);
            if (exports.length === 4) started();
            await pending;
            return '<svg width="10" height="10"></svg>';
        },
        undefined,
        undefined,
        undefined,
        1,
        () => cancelled,
    );
    await exporting;
    cancelled = true;
    release();
    await capture;
    expect(exports).toEqual(["vector:0", "vector:1", "vector:2", "vector:3"]);
    expect(unscheduledReads).toBe(0);
});

test.each(["svg", "png"])(
    "overlapping revisions share four %s export slots and skip obsolete queued work",
    async (format) => {
        const { captureSelectionSource } = await import(
            "../src/plugin/capture"
        );
        const fixture = JSON.parse(
            await readFile("fixtures/source/negative-gap.json", "utf8"),
        );
        const png = new Uint8Array(
            await readFile("fixtures/authored/odd-size.png"),
        );
        const root = {
            ...fixture.root.properties,
            id: fixture.root.id,
            name: fixture.root.name,
            type: fixture.root.type,
            children: Array.from({ length: 6 }, (_, index) => ({
                ...fixture.root.properties,
                id: `vector:${index}`,
                name: `Vector ${index}`,
                type: "VECTOR",
                fills: [],
            })),
        } as unknown as SceneNode;
        let revision = 0;
        let active = 0;
        let peak = 0;
        const exported: number[] = [];
        const releases: (() => void)[] = [];
        let firstStarted: () => void = () => {};
        const started = new Promise<void>((resolve) => {
            firstStarted = resolve;
        });
        const jobs = [];
        for (let current = 1; current <= 5; current++) {
            revision = current;
            const exportNode = async () => {
                exported.push(current);
                peak = Math.max(peak, ++active);
                if (exported.length === 4) firstStarted();
                await new Promise<void>((resolve) => releases.push(resolve));
                active--;
                // A failed host export must release its shared slot too.
                if (current === 1) throw Error("superseded export failed");
            };
            jobs.push(
                captureSelectionSource(
                    [root],
                    Symbol("mixed"),
                    async () => {
                        await exportNode();
                        return '<svg width="10" height="10"></svg>';
                    },
                    undefined,
                    undefined,
                    format === "png"
                        ? async () => {
                              await exportNode();
                              return png;
                          }
                        : undefined,
                    1,
                    () => current !== revision,
                ),
            );
            if (current === 1) await started;
        }
        // Drain deferred host calls without adding real capture delays.
        let done = false;
        const completed = Promise.all(jobs).finally(() => {
            done = true;
        });
        for (let turn = 0; turn < 1_000 && !done; turn++) {
            for (const release of releases.splice(0)) release();
            await Promise.resolve();
        }
        expect(done).toBe(true);
        const results = await completed;
        expect(peak).toBe(4);
        expect(exported).toEqual([1, 1, 1, 1, 5, 5, 5, 5, 5, 5]);
        expect(results.at(-1)).toMatchObject({ ok: true });
        const last = results.at(-1);
        if (last?.ok && !last.empty)
            expect(last.source.root.children?.map((child) => child.id)).toEqual(
                Array.from({ length: 6 }, (_, index) => `vector:${index}`),
            );
    },
);

describe("milestone7", () => {
    test("captures a Section and skips a non-visual child without aborting", async () => {
        const section = {
            type: "SECTION",
            id: "section:1",
            name: "Table section",
            x: 0,
            y: 0,
            width: 400,
            height: 200,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: [],
            strokes: [],
            cornerRadius: 0,
            clipsContent: false,
            children: [
                {
                    type: "SLICE",
                    id: "slice:1",
                    name: "Helper slice",
                    x: 0,
                    y: 0,
                    width: 20,
                    height: 20,
                    opacity: 1,
                    visible: true,
                    rotation: 0,
                },
            ],
        } as unknown as SceneNode;
        const result = await captureSelection([section], Symbol("mixed"));
        expect(result).toMatchObject({
            ok: true,
            snapshot: { root: { kind: "section", children: [] } },
            warnings: [{ code: "NON_VISUAL_NODE_SKIPPED" }],
        });
        const unknownLeaf = await captureSelection(
            [
                {
                    type: "UNKNOWN_LEAF",
                    id: "unknown:leaf",
                    name: "Unknown leaf",
                    x: 0,
                    y: 0,
                    width: 20,
                    height: 20,
                    opacity: 1,
                    visible: true,
                    rotation: 0,
                } as unknown as SceneNode,
            ],
            Symbol("mixed"),
        );
        expect(unknownLeaf).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ code: "PNG_EXPORT_FAILED" }),
            ],
        });
        const structuralTable = {
            type: "TABLE",
            id: "table:1",
            name: "Table fallback",
            x: 0,
            y: 0,
            width: 120,
            height: 80,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: [],
            children: [
                {
                    type: "RECTANGLE",
                    id: "table:cell",
                    name: "Cell",
                    x: 0,
                    y: 0,
                    width: 120,
                    height: 80,
                    opacity: 1,
                    visible: true,
                    rotation: 0,
                    fills: [],
                    strokes: [],
                    strokeWeight: 0,
                    strokeAlign: "INSIDE",
                    cornerRadius: 0,
                },
            ],
        } as unknown as SceneNode;
        const tableResult = await captureSelection(
            [structuralTable],
            Symbol("mixed"),
        );
        expect(tableResult).toMatchObject({
            ok: true,
            snapshot: {
                root: { kind: "container", children: [{ kind: "rectangle" }] },
            },
            warnings: [
                expect.objectContaining({
                    code: "CONTAINER_TYPE_APPROXIMATED",
                }),
            ],
        });
        const apiTable = {
            type: "TABLE",
            id: "table:api",
            name: "API table",
            x: 0,
            y: 0,
            width: 160,
            height: 48,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: [],
            numRows: 1,
            numColumns: 2,
            cellAt(row: number, column: number) {
                return {
                    type: "TABLE_CELL",
                    rowIndex: row,
                    columnIndex: column,
                    width: 80,
                    height: 48,
                    fills: [
                        {
                            type: "SOLID",
                            visible: true,
                            opacity: 1,
                            color: { r: 0.95, g: 0.95, b: 0.95 },
                        },
                    ],
                    text: {
                        characters: column === 0 ? "Name" : "Value",
                        fontName: { family: "Inter", style: "Regular" },
                        fontSize: 14,
                        fontWeight: 400,
                        fills: [
                            {
                                type: "SOLID",
                                visible: true,
                                opacity: 1,
                                color: { r: 0, g: 0, b: 0 },
                            },
                        ],
                    },
                };
            },
        } as unknown as SceneNode;
        const apiTableResult = await captureSelection(
            [apiTable],
            Symbol("mixed"),
        );
        expect(apiTableResult).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: "container",
                    children: [
                        { kind: "container", children: [{ kind: "text" }] },
                        { kind: "container", children: [{ kind: "text" }] },
                    ],
                },
            },
            warnings: expect.arrayContaining([
                expect.objectContaining({ code: "TABLE_LAYOUT_APPROXIMATED" }),
            ]),
        });
        const apiTableWithoutNodeGeometry = {
            type: "TABLE",
            id: "table:missing-geometry",
            name: "API table without node geometry",
            fills: [],
            numRows: 1,
            numColumns: 1,
            cellAt() {
                return {
                    type: "TABLE_CELL",
                    rowIndex: 0,
                    columnIndex: 0,
                    width: 96,
                    height: 32,
                    fills: [],
                    text: { characters: "Cell", fills: [] },
                };
            },
        } as unknown as SceneNode;
        const tableWithoutNodeGeometryResult = await captureSelection(
            [apiTableWithoutNodeGeometry],
            Symbol("mixed"),
        );
        expect(tableWithoutNodeGeometryResult).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: "container",
                    width: 96,
                    height: 32,
                    children: [
                        { kind: "container", children: [{ kind: "text" }] },
                    ],
                },
            },
        });
        const corruptSection = {
            type: "SECTION",
            id: "section:invalid-geometry",
            name: "Corrupt section",
            x: 0,
            y: 0,
            width: Number.NaN,
            height: 200,
            fills: [],
            strokes: [],
            children: [],
        } as unknown as SceneNode;
        const corruptSectionResult = await captureSelection(
            [corruptSection],
            Symbol("mixed"),
        );
        expect(corruptSectionResult).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ code: "INVALID_GEOMETRY" }),
            ],
        });
        const standaloneCell = {
            type: "TABLE_CELL",
            id: "table:standalone-cell",
            name: "Standalone cell",
            rowIndex: 0,
            columnIndex: 0,
            width: 80,
            height: 48,
            fills: [],
            text: {
                characters: "Cell",
                fontName: { family: "Inter", style: "Regular" },
                fontSize: 14,
                fontWeight: 400,
                fills: [],
            },
        } as unknown as SceneNode;
        const standaloneCellResult = await captureSelection(
            [standaloneCell],
            Symbol("mixed"),
        );
        expect(standaloneCellResult).toMatchObject({
            ok: true,
            snapshot: {
                root: { kind: "container", children: [{ kind: "text" }] },
            },
        });
        const connector = {
            type: "CONNECTOR",
            id: "connector:1",
            name: "Flattened connector",
            x: 0,
            y: 0,
            width: 120,
            height: 80,
            opacity: 1,
            visible: true,
            rotation: 0,
        } as unknown as SceneNode;
        const connectorResult = await captureSelection(
            [connector],
            Symbol("mixed"),
            async (node) =>
                `<svg viewBox="0 0 120 80"><path data-node="${node.id}" /></svg>`,
        );
        expect(connectorResult).toMatchObject({
            ok: true,
            snapshot: { root: { kind: "svg", sourceType: "CONNECTOR" } },
            warnings: [
                expect.objectContaining({ code: "SVG_FLATTENED_APPROXIMATED" }),
            ],
        });
    });
});

describe("milestone7", () => {
    test("approximates structural geometry fields omitted by Figma typings", async () => {
        const section = {
            type: "SECTION",
            id: "section:missing-common-fields",
            name: "Section without common fields",
            x: 0,
            y: 0,
            width: 400,
            height: 200,
            fills: [],
            strokes: [],
            children: [],
        } as unknown as SceneNode;
        const result = await captureSelection([section], Symbol("mixed"));
        expect(result).toMatchObject({
            ok: true,
            snapshot: {
                root: {
                    kind: "section",
                    opacity: 1,
                    rotation: 0,
                    visible: true,
                },
            },
        });
    });
});

describe("milestone7", () => {
    test("captures layered paints, asymmetric strokes, and ordered effects", async () => {
        const rectangle = {
            type: "RECTANGLE",
            id: "resilience:rectangle",
            name: "Asymmetric cell",
            x: 0,
            y: 0,
            width: 180,
            height: 80,
            opacity: 1,
            visible: true,
            rotation: 0,
            fills: [
                {
                    type: "SOLID",
                    visible: true,
                    opacity: 1,
                    color: { r: 0.1, g: 0.2, b: 0.3 },
                },
                {
                    type: "GRADIENT_LINEAR",
                    visible: true,
                    opacity: 0.5,
                    gradientTransform: [
                        [1, 0, 0],
                        [0, 1, 0],
                    ],
                    gradientStops: [
                        { position: 0, color: { r: 1, g: 0, b: 0 } },
                        { position: 0.5, color: { r: 0, g: 1, b: 0 } },
                        { position: 1, color: { r: 0, g: 0, b: 1 } },
                    ],
                },
            ],
            strokes: [
                {
                    type: "GRADIENT_LINEAR",
                    visible: true,
                    opacity: 1,
                    gradientTransform: [
                        [1, 0, 0],
                        [0, 1, 0],
                    ],
                    gradientStops: [
                        { position: 0, color: { r: 1, g: 1, b: 0 } },
                        { position: 0.5, color: { r: 0, g: 1, b: 1 } },
                        { position: 1, color: { r: 1, g: 0, b: 1 } },
                    ],
                },
                {
                    type: "SOLID",
                    visible: true,
                    opacity: 1,
                    color: { r: 1, g: 1, b: 1 },
                },
            ],
            strokeWeight: Symbol("figma.mixed"),
            strokeTopWeight: 1,
            strokeRightWeight: 4,
            strokeBottomWeight: 2,
            strokeLeftWeight: 3,
            strokeAlign: "CENTER",
            dashPattern: [4, 2],
            cornerRadius: 8,
            clipsContent: true,
            effects: [
                {
                    type: "DROP_SHADOW",
                    visible: true,
                    blendMode: "NORMAL",
                    color: { r: 0, g: 0, b: 0, a: 0.25 },
                    offset: { x: 0, y: 2 },
                    radius: 4,
                    spread: 1,
                },
                {
                    type: "DROP_SHADOW",
                    visible: true,
                    blendMode: "NORMAL",
                    color: { r: 0, g: 0, b: 1, a: 0.2 },
                    offset: { x: 1, y: 1 },
                    radius: 2,
                    spread: 0,
                },
                {
                    type: "INNER_SHADOW",
                    visible: true,
                    blendMode: "NORMAL",
                    color: { r: 0, g: 0, b: 0, a: 0.2 },
                    offset: { x: 0, y: 1 },
                    radius: 2,
                },
            ],
        } as unknown as SceneNode;
        const result = await captureSelection(
            [rectangle],
            Symbol("figma.mixed"),
        );
        expect(result.ok).toBe(true);
        if (!result.ok || result.empty) return;
        expect(result.snapshot.root.kind).toBe("rectangle");
        if (result.snapshot.root.kind !== "rectangle") return;
        expect(result.snapshot.root.fills).toHaveLength(2);
        expect(result.snapshot.root.strokes[0]).toMatchObject({
            strokeTopWeight: 1,
            strokeRightWeight: 4,
            strokeBottomWeight: 2,
            strokeLeftWeight: 3,
            dashPattern: [4, 2],
        });
        expect(
            result.snapshot.root.shadows.map((shadow) => shadow.spread),
        ).toEqual([1, 0, 0]);
        expect(result.warnings.map((warning) => warning.code)).toEqual(
            expect.arrayContaining([
                "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED",
                "DASHED_STROKE_APPROXIMATED",
                "INNER_SHADOW_APPROXIMATED",
            ]),
        );
        expect(
            result.warnings.find(
                (warning) =>
                    warning.code === "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED" &&
                    warning.propertyPath === "strokes",
            )?.message,
        ).toContain("only the first supported stroke paint");
        expect(
            result.warnings.find(
                (warning) =>
                    warning.code === "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED" &&
                    warning.propertyPath === "fills",
            )?.message,
        ).toContain("all supported layers are preserved");
        const mixedPaintResult = await captureSelection(
            [
                {
                    ...rectangle,
                    fills: Symbol("figma.mixed"),
                    strokes: [],
                } as unknown as SceneNode,
            ],
            Symbol("figma.mixed"),
        );
        expect(mixedPaintResult).toMatchObject({
            ok: true,
            warnings: expect.arrayContaining([
                expect.objectContaining({ code: "MIXED_PAINT_APPROXIMATED" }),
            ]),
        });
        const imageStrokeResult = await captureSelection(
            [
                {
                    ...rectangle,
                    id: "resilience:image-stroke",
                    fills: [],
                    effects: [],
                    strokes: [
                        {
                            type: "IMAGE",
                            visible: true,
                            opacity: 1,
                            imageHash: "image-stroke",
                            scaleMode: "FILL",
                        },
                    ],
                } as unknown as SceneNode,
            ],
            Symbol("figma.mixed"),
        );
        expect(imageStrokeResult).toMatchObject({
            ok: true,
            snapshot: { root: { kind: "rectangle", strokes: [] } },
            warnings: [
                expect.objectContaining({ code: "UNSUPPORTED_STROKE_PAINT" }),
            ],
        });
    });
});
