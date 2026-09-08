// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { CaptureResult, Diagnostic, SnapshotNode } from "./snapshot";
import type { SourceCapture, SourceBytes } from "./source";

type ComponentProperty = {
    type: "TEXT" | "BOOLEAN" | "INSTANCE_SWAP";
    defaultValue: string | boolean;
};
export type ComponentContract = {
    version: 1;
    properties: Record<string, ComponentProperty>;
    bindings: Record<
        string,
        { characters?: string; visible?: string; mainComponent?: string }
    >;
};

/** Versioned, host-independent component graph shared by capture and conversion. */
export type ComponentLibrary<Node> = {
    version: 1;
    definitions: {
        id: string;
        name: string;
        scope?: "complete" | "private";
        axes: Record<string, { defaultValue: string; options: string[] }>;
        contract?: ComponentContract;
        variants: { id: string; values: Record<string, string>; root: Node }[];
    }[];
    references: Record<
        string,
        {
            definitionId: string;
            variantId: string;
            properties?: Record<string, string | boolean>;
        }
    >;
};

export function validateComponentLibrary<Node>(
    value: unknown,
    validateNode: (node: Node) => void,
): asserts value is ComponentLibrary<Node> {
    const object = (v: unknown): v is Record<string, unknown> =>
        typeof v === "object" && v !== null && !Array.isArray(v);
    const fail = (): never => {
        throw Error("Invalid component library contract");
    };
    if (
        !object(value) ||
        value.version !== 1 ||
        !Array.isArray(value.definitions) ||
        !object(value.references)
    )
        fail();
    const library = value as ComponentLibrary<Node>;
    const definitions = new Map<string, Set<string>>();
    for (const d of library.definitions) {
        if (
            !object(d) ||
            typeof d.id !== "string" ||
            !d.id ||
            definitions.has(d.id) ||
            typeof d.name !== "string" ||
            (d.scope !== undefined &&
                d.scope !== "complete" &&
                d.scope !== "private") ||
            !object(d.axes) ||
            !Array.isArray(d.variants) ||
            !d.variants.length
        )
            fail();
        for (const axis of Object.values(d.axes)) {
            if (
                !object(axis) ||
                typeof axis.defaultValue !== "string" ||
                !Array.isArray(axis.options) ||
                !axis.options.length ||
                axis.options.some((v) => typeof v !== "string") ||
                new Set(axis.options).size !== axis.options.length ||
                !axis.options.includes(axis.defaultValue)
            )
                fail();
        }
        if (d.contract !== undefined) {
            const c = d.contract;
            if (
                !object(c) ||
                c.version !== 1 ||
                !object(c.properties) ||
                !object(c.bindings)
            )
                fail();
            for (const p of Object.values(c.properties)) {
                if (
                    !object(p) ||
                    !["TEXT", "BOOLEAN", "INSTANCE_SWAP"].includes(p.type) ||
                    typeof p.defaultValue !==
                        (p.type === "BOOLEAN" ? "boolean" : "string")
                )
                    fail();
            }
            for (const refs of Object.values(c.bindings)) {
                if (!object(refs)) fail();
                for (const [field, key] of Object.entries(refs)) {
                    const expected = {
                        characters: "TEXT",
                        visible: "BOOLEAN",
                        mainComponent: "INSTANCE_SWAP",
                    }[field];
                    if (
                        !expected ||
                        typeof key !== "string" ||
                        c.properties[key]?.type !== expected
                    )
                        fail();
                }
            }
        }
        const ids = new Set<string>();
        const tuples = new Set<string>();
        for (const v of d.variants) {
            if (
                !object(v) ||
                typeof v.id !== "string" ||
                !v.id ||
                ids.has(v.id) ||
                !object(v.values) ||
                Object.keys(v.values).length !== Object.keys(d.axes).length
            )
                fail();
            for (const [key, axis] of Object.entries(d.axes))
                if (!axis.options.includes(v.values[key])) fail();
            const tuple = JSON.stringify(
                Object.keys(d.axes)
                    .sort()
                    .map((k) => v.values[k]),
            );
            if (tuples.has(tuple)) fail();
            tuples.add(tuple);
            ids.add(v.id);
            validateNode(v.root);
        }
        definitions.set(d.id, ids);
    }
    for (const r of Object.values(library.references))
        if (
            !object(r) ||
            typeof r.definitionId !== "string" ||
            typeof r.variantId !== "string" ||
            !definitions.get(r.definitionId)?.has(r.variantId) ||
            (r.properties !== undefined &&
                (!object(r.properties) ||
                    Object.values(r.properties).some(
                        (v) => typeof v !== "string" && typeof v !== "boolean",
                    )))
        )
            fail();
}

export type ReachabilityNode = {
    id: string;
    children?: ReachabilityNode[];
};

export type ReachabilitySelection = {
    kind: "component-set" | "component" | "instance" | "frame";
    definitionId?: string;
};

export type ReachabilityResult = {
    variants: ReadonlyMap<string, ReadonlySet<string>>;
    completeFamilies: ReadonlySet<string>;
    axes: ReadonlyMap<
        string,
        ReadonlyMap<
            string,
            { options: readonly string[]; defaultValue: string }
        >
    >;
    diagnostics: readonly string[];
};

export function planReachableVariants<Node extends ReachabilityNode>(
    library: ComponentLibrary<Node>,
    root: Node,
    selection: ReachabilitySelection,
): ReachabilityResult {
    const definitions = new Map(library.definitions.map((d) => [d.id, d]));
    const variants = new Map<string, Set<string>>();
    const completeFamilies = new Set<string>();
    const diagnostics: string[] = [];
    const visitedVariants = new Set<string>();
    const visitedNodes = new Set<string>();

    const full = (id: string, reason: string) => {
        const definition = definitions.get(id);
        if (!definition) {
            diagnostics.push(`${reason}: unknown component family ${id}`);
            return;
        }
        completeFamilies.add(id);
        variants.set(id, new Set(definition.variants.map((v) => v.id)));
        for (const variant of definition.variants)
            retain(id, variant.id, `complete family ${id}`);
    };
    const retain = (
        definitionId: string,
        variantId: string,
        reason: string,
    ) => {
        const definition = definitions.get(definitionId);
        if (!definition) {
            diagnostics.push(
                `${reason}: unknown component family ${definitionId}`,
            );
            return;
        }
        const variant = definition.variants.find((v) => v.id === variantId);
        if (!variant) {
            full(definitionId, `${reason}: unknown variant ${variantId}`);
            diagnostics.push(`${reason}: unknown variant ${variantId}`);
            return;
        }
        if (!completeFamilies.has(definitionId)) {
            const retained = variants.get(definitionId) ?? new Set<string>();
            retained.add(variant.id);
            variants.set(definitionId, retained);
        }
        const key = `${definitionId}/${variant.id}`;
        if (visitedVariants.has(key)) return;
        visitedVariants.add(key);
        visitNode(variant.root);
    };
    const visitNode = (node: Node) => {
        if (visitedNodes.has(node.id)) return;
        visitedNodes.add(node.id);
        const reference = library.references[node.id];
        if (reference) {
            retain(
                reference.definitionId,
                reference.variantId,
                `reference ${node.id}`,
            );
            const definition = definitions.get(reference.definitionId);
            if (
                definition?.contract &&
                Object.values(definition.contract.properties).some(
                    (p) => p.type === "INSTANCE_SWAP",
                )
            )
                full(definition.id, `instance-swap contract ${definition.id}`);
        }
        node.children?.forEach((child) => {
            visitNode(child as Node);
        });
    };

    if (selection.kind === "component-set" || selection.kind === "component") {
        if (selection.definitionId)
            full(selection.definitionId, `public ${selection.kind}`);
        else
            diagnostics.push(`public ${selection.kind}: missing definition id`);
    }
    visitNode(root);

    const axes = new Map<
        string,
        Map<string, { options: readonly string[]; defaultValue: string }>
    >();
    for (const definition of library.definitions) {
        const retained = variants.get(definition.id);
        if (!retained || completeFamilies.has(definition.id)) continue;
        const axisResult = new Map<
            string,
            { options: readonly string[]; defaultValue: string }
        >();
        for (const [axisName, axis] of Object.entries(definition.axes)) {
            const options = [
                ...new Set(
                    definition.variants
                        .filter((v) => retained.has(v.id))
                        .map((v) => v.values[axisName]),
                ),
            ].sort();
            if (options.length)
                axisResult.set(axisName, {
                    options,
                    defaultValue: options.includes(axis.defaultValue)
                        ? axis.defaultValue
                        : options[0],
                });
        }
        axes.set(definition.id, axisResult);
    }
    return {
        variants,
        completeFamilies,
        axes,
        diagnostics: [...diagnostics].sort(),
    };
}

/** Resolve each definition using precisely the same offline visual normalizer. */
export async function normalizeComponents(
    source: SourceCapture<SourceBytes>,
    normalize: (source: SourceCapture<SourceBytes>) => Promise<CaptureResult>,
): Promise<{
    library: ComponentLibrary<SnapshotNode>;
    warnings: Diagnostic[];
}> {
    const library: ComponentLibrary<SnapshotNode> = {
        version: 1,
        definitions: [],
        references: source.components?.references ?? {},
    };
    const warnings: Diagnostic[] = [];
    for (const definition of source.components?.definitions ?? []) {
        const variants = [];
        for (const variant of definition.variants) {
            const result = await normalize({
                ...source,
                components: undefined,
                root: visiblePropertyTree(
                    variant.root,
                    definition.contract?.bindings ?? {},
                ),
            });
            if (!result.ok || result.empty)
                throw Error(
                    `Cannot normalize component variant ${definition.name} (${variant.id})`,
                );
            warnings.push(...result.warnings);
            variants.push({ ...variant, root: result.snapshot.root });
        }
        library.definitions.push({ ...definition, variants });
    }
    return { library, warnings };
}

function visiblePropertyTree<Node extends SourceCapture<SourceBytes>["root"]>(
    node: Node,
    bindings: NonNullable<
        ComponentLibrary<Node>["definitions"][number]["contract"]
    >["bindings"],
): Node {
    return {
        ...node,
        properties: bindings[node.id]?.visible
            ? { ...node.properties, visible: true }
            : node.properties,
        ...(node.children
            ? {
                  children: node.children.map((child) =>
                      visiblePropertyTree(child, bindings),
                  ),
              }
            : {}),
    };
}
