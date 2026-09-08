// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    axisScalar,
    appearanceDefault,
    appearanceStates,
    type Appearance,
    declaration,
    optionValue,
    predicate,
    selectValues,
    type VariantAxis,
} from "./component-variants";
import {
    childOrder,
    childRoles,
    structuralSignature,
} from "./component-structure";
import {
    buttonBehavior,
    type ComponentGenerationOptions,
} from "./component-behavior";
import type { VariableLibrary } from "../plugin/variable-library";
import { componentTokens } from "./component-tokens";
import type { ComponentLibrary, ComponentContract } from "../plugin/components";
import type { Diagnostic, SnapshotNode } from "../plugin/snapshot";
import {
    typeName,
    reservedTypeNames,
    enumReservedNames,
} from "./component-names";
import {
    binding,
    openElement,
    closeElement,
    requireValue,
    elementTree,
    literal,
    printLine,
    treeLines,
    type Binding,
    type Element,
    type SlintLine,
} from "./slint-ir";
import { identifier, nameAllocator } from "./component-names";
import { reference } from "./slint-ir";

type Definition = ComponentLibrary<SnapshotNode>["definitions"][number];
type ComponentUse = {
    name: string;
    bindings: Binding[];
    templateBindings: Binding[];
};
export type ComponentUses = Map<SnapshotNode, ComponentUse>;
type Sample = { tree: Element; values: Record<string, string> };
const defaults: Record<string, string> = {
    background: "transparent",
    "border-color": "transparent",
    "border-width": "0px",
    "border-radius": "0px",
    opacity: "1",
    clip: "false",
    "font-italic": "false",
};
function elementKey(element: Element): string {
    return `${element.type}:${element.origin?.name ?? "helper"}`;
}
function contractNames(d: Definition) {
    const allocate = nameAllocator([
        "valid-variant",
        ...Object.keys(d.axes).map((a) => `variant-${identifier(a)}`),
    ]);
    return new Map(
        Object.entries(d.contract?.properties ?? {})
            .sort(([a], [b]) => (a < b ? -1 : 1))
            .filter(([, p]) => p.type !== "INSTANCE_SWAP")
            .map(([key, p]) => [
                key,
                {
                    name: allocate(identifier(key.split("#")[0])),
                    type: p.type === "BOOLEAN" ? "bool" : "string",
                    property: p,
                },
            ]),
    );
}
/** Bind authored properties by node provenance; instances may match the definition's semantic child path. */
function applyContract(
    tree: Element,
    contract: ComponentContract | undefined,
    names: ReturnType<typeof contractNames>,
    model?: Element,
): Element {
    const refs = contract?.bindings[model?.origin?.id ?? tree.origin?.id ?? ""];
    const result: Element = {
        ...tree,
        bindings: tree.bindings.map((b) => {
            const property =
                b.name === "text" && refs?.characters
                    ? names.get(refs.characters)
                    : undefined;
            return property && b.value.kind !== "raw"
                ? {
                      ...b,
                      value: reference(property.type, `root.${property.name}`),
                  }
                : b;
        }),
        children: tree.children.map((child, i) =>
            applyContract(
                child,
                contract,
                names,
                model?.children[i] &&
                    elementKey(model.children[i]) === elementKey(child)
                    ? model.children[i]
                    : undefined,
            ),
        ),
    };
    if (refs?.visible) {
        const property = names.get(refs.visible);
        if (property) result.condition = `root.${property.name}`;
    }
    return result;
}
function normalizeBindingOrder(tree: Element): Element {
    return {
        ...tree,
        bindings: [...tree.bindings].sort((a, b) =>
            a.name < b.name ? -1 : a.name > b.name ? 1 : 0,
        ),
        children: tree.children.map(normalizeBindingOrder),
    };
}

/** Compile definitions independently of occurrences, with private literal specializations. */
export function generateComponents(
    root: SnapshotNode,
    library: ComponentLibrary<SnapshotNode> | undefined,
    render: (node: SnapshotNode, uses: ComponentUses) => SlintLine[],
    variables?: VariableLibrary,
    options: ComponentGenerationOptions = {},
): { source: string[]; uses: ComponentUses; warnings: Diagnostic[] } {
    const uses: ComponentUses = new Map();
    const source: string[] = [];
    const warnings: Diagnostic[] = [];
    if (!library) return { source, uses, warnings };
    const graph = library;
    const definitions = new Map(library.definitions.map((d) => [d.id, d]));
    const bases = [...definitions.keys()]
        .sort()
        .map(
            (id) =>
                [id, typeName(requireValue(definitions.get(id)).name)] as const,
        );
    const allocateType = nameAllocator(
        [...reservedTypeNames, ...bases.map(([, name]) => name)],
        "",
    );
    const claimed = new Set(reservedTypeNames);
    const names = new Map(
        bases.map(([id, base]) => {
            const name = claimed.has(base) ? allocateType(base) : base;
            claimed.add(base);
            return [id, name];
        }),
    );
    const tokens = componentTokens(variables, allocateType);
    const assets = componentAssets(allocateType("Assets"));
    const occurrences = new Map<string, SnapshotNode[]>();
    function collect(node: SnapshotNode) {
        const ref = graph.references[node.id];
        if (ref) {
            const group = occurrences.get(ref.definitionId) ?? [];
            group.push(node);
            occurrences.set(ref.definitionId, group);
            return;
        }
        if ("children" in node) node.children.forEach(collect);
    }
    collect(root);
    // Validate dependency cycles even when appearance-only dependencies are inlined.
    const checked = new Set<string>(),
        active = new Set<string>();
    function check(id: string, variantId: string) {
        const key = `${id}/${variantId}`;
        if (active.has(key)) throw Error(`Cyclic component dependency: ${key}`);
        if (checked.has(key)) return;
        const d = definitions.get(id);
        if (!d) throw Error(`Missing component definition: ${id}`);
        const variant = d.variants.find((v) => v.id === variantId);
        if (!variant) throw Error(`Missing component variant: ${variantId}`);
        active.add(key);
        function nested(n: SnapshotNode) {
            const ref = graph.references[n.id];
            if (ref) check(ref.definitionId, ref.variantId);
            else if ("children" in n) n.children.forEach(nested);
        }
        if ("children" in variant.root) variant.root.children.forEach(nested);
        active.delete(key);
        checked.add(key);
    }
    for (const id of [...occurrences.keys()].sort()) {
        const d = definitions.get(id);
        if (!d) throw Error(`Missing component definition: ${id}`);
        for (const v of d.variants) check(id, v.id);
    }
    const emptyUses: ComponentUses = new Map();
    const rendered = (node: SnapshotNode) =>
        normalizeBindingOrder(
            assets.apply(tokens.apply(elementTree(render(node, emptyUses)))),
        );
    const preparedTrees = new Map<SnapshotNode, Element>();
    // Allocate family assets before processing instance overrides. Adding an
    // occurrence must not rename assets referenced by a later family.
    for (const id of [...occurrences.keys()].sort()) {
        const definition = requireValue(definitions.get(id));
        for (const variant of [...definition.variants].sort((a, b) =>
            a.id < b.id ? -1 : 1,
        ))
            preparedTrees.set(variant.root, rendered(variant.root));
    }
    for (const id of [...occurrences.keys()].sort()) {
        const d = requireValue(definitions.get(id));
        const name = requireValue(names.get(id));
        const allocateProperty = nameAllocator(["valid-variant"]);
        const axes: VariantAxis[] = Object.entries(d.axes)
            .sort(([a], [b]) => (a < b ? -1 : 1))
            .map(([key, axis]) => {
                const allocateOption = nameAllocator(enumReservedNames);
                const scalar = axisScalar(axis.options);
                return {
                    key,
                    property: allocateProperty(`variant-${identifier(key)}`),
                    type: scalar ?? allocateType(`${name}${typeName(key)}`),
                    options: new Map(
                        [...axis.options]
                            .sort()
                            .map((option) => [
                                option,
                                scalar === "bool"
                                    ? option.toLowerCase()
                                    : scalar === "int"
                                      ? option
                                      : allocateOption(identifier(option)),
                            ]),
                    ),
                };
            });
        const isDefault = (v: Definition["variants"][number]) =>
            Object.entries(d.axes).every(
                ([key, axis]) => v.values[key] === axis.defaultValue,
            );
        const variants = [...d.variants].sort(
            (a, b) =>
                Number(isDefault(b)) - Number(isDefault(a)) ||
                (a.id < b.id ? -1 : 1),
        );
        const publicNames = contractNames(d);
        for (const p of publicNames.values()) allocateProperty(p.name);
        const variantTrees = new Map(
            variants.map((v) => [
                v.id,
                requireValue(preparedTrees.get(v.root)),
            ]),
        );
        const samples: Sample[] = variants.map((v) => ({
            values: v.values,
            tree: applyContract(
                requireValue(variantTrees.get(v.id)),
                d.contract,
                publicNames,
            ),
        }));
        for (const [key, p] of publicNames) {
            const mapped = (tree: Element): boolean =>
                tree.condition === `root.${p.name}` ||
                tree.bindings.some(
                    (b) =>
                        b.value.kind === "reference" &&
                        b.value.code === `root.${p.name}`,
                ) ||
                tree.children.some(mapped);
            if (!samples.some((s) => mapped(s.tree)))
                warnings.push({
                    severity: "warning",
                    code: "COMPONENT_PROPERTY_STATIC",
                    nodeId: d.id,
                    message: `${d.name}.${key}: the authored property has no editable native binding in this capture (for example, rasterized text)`,
                });
        }
        const behavior = buttonBehavior(
            options.behaviors?.[id],
            axes,
            publicNames,
        );
        const declarations: string[] = [...(behavior?.declarations ?? [])];
        for (const a of axes) {
            if (a.type !== "int" && a.type !== "bool")
                source.push(
                    ...declaration(
                        `export enum ${a.type} { `,
                        [...a.options.values()].join(", "),
                        " }",
                    ),
                );
            else if (a.type === "int")
                declarations.push(
                    `    // ${a.key}: allowed values ${[...a.options.keys()].sort((x, y) => Number(x) - Number(y)).join(", ")}.`,
                );
            declarations.push(
                ...declaration(
                    `    in property <${a.type}> ${a.property}: `,
                    behavior?.axis === a.key
                        ? behavior.expression
                        : optionValue(a, d.axes[a.key].defaultValue),
                ),
            );
        }
        for (const [key, p] of publicNames) {
            const findDefault = (tree: Element): string | undefined => {
                if (
                    p.type === "string" &&
                    d.contract?.bindings[tree.origin?.id ?? ""]?.characters ===
                        key
                )
                    return tree.bindings.find(
                        (b) => b.name === "text" && b.value.kind !== "raw",
                    )?.value.code;
                for (const child of tree.children) {
                    const value = findDefault(child);
                    if (value !== undefined) return value;
                }
            };
            declarations.push(
                ...declaration(
                    `    in property <${p.type}> ${p.name}: `,
                    selectValues(
                        variants.map((v) => ({
                            values: v.values,
                            code:
                                findDefault(
                                    requireValue(variantTrees.get(v.id)),
                                ) ?? JSON.stringify(p.property.defaultValue),
                        })),
                        axes,
                        JSON.stringify(p.property.defaultValue),
                    ),
                ),
            );
        }
        for (const [key, p] of Object.entries(d.contract?.properties ?? {}))
            if (p.type === "INSTANCE_SWAP")
                warnings.push({
                    severity: "warning",
                    code: "COMPONENT_SWAP_STATIC",
                    nodeId: d.id,
                    message: `${d.name}.${key}: instance swaps retain the captured appearance; a runtime component mapping is required`,
                });
        const validity = predicate(
            samples.map((s) => s.values),
            axes,
        );
        if (validity !== "true") {
            declarations.push(
                ...declaration(
                    "    private property <bool> valid-variant: ",
                    validity,
                ),
            );
            warnings.push({
                severity: "warning",
                code: "COMPONENT_VARIANT_DOMAIN",
                nodeId: d.id,
                message: `${d.name}: only the ${samples.length} captured variant combinations are supported; other values render empty`,
            });
        }
        // Let auto-layout contribute intrinsic dimensions. Freeform definitions
        // still need their captured preferred dimensions.
        for (const axis of ["width", "height"] as const) {
            const sizing =
                axis === "width"
                    ? "layoutSizingHorizontal"
                    : "layoutSizingVertical";
            if (
                variants.every(
                    (v) =>
                        "autoLayout" in v.root &&
                        v.root.autoLayout &&
                        v.root[sizing] === "hug",
                )
            )
                continue;
            declarations.push(
                ...declaration(
                    `    preferred-${axis}: `,
                    selectValues(
                        variants.map((v) => ({
                            values: v.values,
                            code: `${v.root[axis]}px`,
                        })),
                        axes,
                        `${variants[0].root[axis]}px`,
                    ),
                ),
            );
        }
        let serial = 0;
        function emitTree(
            group: Sample[],
            depth: number,
            rootElement = false,
            extraCondition?: string,
            inLayout = false,
        ): string[] {
            const first = group[0].tree;
            // Different element kinds cannot share identity.
            const bindingNames = [
                ...new Set(
                    group.flatMap((s) => s.tree.bindings.map((b) => b.name)),
                ),
            ].sort();
            const compatible =
                group.every(
                    (s) =>
                        s.tree.type === first.type &&
                        s.tree.condition === first.condition,
                ) &&
                bindingNames.every((field) => {
                    const entries = group.map((s) =>
                        s.tree.bindings.find((b) => b.name === field),
                    );
                    const present = entries.filter((b) => b !== undefined);
                    if (
                        entries.some((b) => !b) &&
                        defaults[field] === undefined
                    )
                        return false;
                    return (
                        present.every(
                            (b) => b.value.type === present[0].value.type,
                        ) &&
                        (present.every((b) => b.value.kind !== "raw") ||
                            entries.every(
                                (b) =>
                                    b &&
                                    JSON.stringify(b.value) ===
                                        JSON.stringify(present[0].value),
                            ))
                    );
                });
            // Merge only a consistent child order. Unique semantic roles establish
            // correspondence; ambiguous repeated roles retain separate subtrees.
            const childKeys = childRoles(group.map((s) => s.tree));
            const order = childOrder(childKeys);
            const orderedKeys = order ?? [];
            const ordered = order !== undefined;
            // Conditional elements only contribute intrinsic size when they are
            // layout items. Keep a layout owner outside structural alternatives,
            // including nested layout alternatives in otherwise shared containers.
            const split = !compatible || !ordered;
            if (
                !inLayout &&
                ((rootElement && split) ||
                    (first.type === "FlexboxLayout" &&
                        (split || extraCondition || first.condition)))
            ) {
                const condition =
                    [
                        extraCondition,
                        rootElement && validity !== "true"
                            ? "root.valid-variant"
                            : undefined,
                    ]
                        .filter(Boolean)
                        .join(" && ") || undefined;
                return [
                    printLine(openElement("FlexboxLayout", depth)),
                    ...[
                        binding("padding", "0px", depth + 1),
                        binding("spacing", "0px", depth + 1),
                        binding("alignment", "stretch", depth + 1),
                        binding("cross-axis-alignment", "stretch", depth + 1),
                    ].map(printLine),
                    ...emitTree(group, depth + 1, false, condition, true),
                    printLine(closeElement(depth)),
                ];
            }
            if (split) {
                const alternatives = new Map<string, Sample[]>();
                for (const sample of group) {
                    const tree = sample.tree;
                    const keys = tree.children.map(elementKey);
                    const sig =
                        new Set(keys).size !== keys.length
                            ? structuralSignature(tree)
                            : JSON.stringify({
                                  type: tree.type,
                                  condition: tree.condition,
                                  bindings: tree.bindings.map((b) => [
                                      b.name,
                                      b.value.type,
                                      b.value.kind === "raw"
                                          ? b.value.code
                                          : null,
                                  ]),
                                  children: keys,
                              });
                    const list = alternatives.get(sig) ?? [];
                    list.push(sample);
                    alternatives.set(sig, list);
                }
                return [...alternatives.values()].flatMap((list) => {
                    const condition = [
                        predicate(
                            list.map((s) => s.values),
                            axes,
                        ),
                        extraCondition,
                        rootElement && validity !== "true"
                            ? "root.valid-variant"
                            : undefined,
                        list[0].tree.condition,
                    ]
                        .filter(Boolean)
                        .map((part) => `(${part})`)
                        .join(" && ");
                    if (list.length < group.length && list.length > 1)
                        return emitTree(
                            list,
                            depth,
                            false,
                            condition,
                            inLayout,
                        );
                    // If correspondence is still ambiguous, preserve every exact
                    // tree. Never substitute the first sample for a whole bucket.
                    const exact = new Map<string, Sample[]>();
                    for (const sample of list) {
                        const sig = structuralSignature(sample.tree);
                        const matches = exact.get(sig) ?? [];
                        matches.push(sample);
                        exact.set(sig, matches);
                    }
                    return [...exact.values()].flatMap((matches) => {
                        const guard =
                            exact.size === 1
                                ? condition
                                : `(${condition}) && (${predicate(
                                      matches.map((s) => s.values),
                                      axes,
                                  )})`;
                        const lines = treeLines(matches[0].tree, depth).map(
                            printLine,
                        );
                        lines[0] = `${"    ".repeat(depth)}if ${guard}: ${matches[0].tree.type} {`;
                        return lines;
                    });
                });
            }
            const indent = "    ".repeat(depth);
            const lines = rootElement
                ? []
                : [
                      `${indent}${extraCondition || first.condition ? `if ${[extraCondition, first.condition].filter(Boolean).join(" && ")}: ` : ""}${first.type} {`,
                  ];
            const bodyDepth = rootElement ? depth : depth + 1;
            const rootVisible =
                [
                    validity !== "true" ? "root.valid-variant" : undefined,
                    first.condition,
                ]
                    .filter(Boolean)
                    .map((part) => `(${part})`)
                    .join(" && ") || "true";
            if (
                rootElement &&
                rootVisible !== "true" &&
                !bindingNames.includes("opacity")
            )
                lines.push(`    opacity: ${rootVisible} ? 1 : 0;`);
            const appearances: Appearance[] = group.map((s) => ({
                values: s.values,
                properties: {},
            }));
            for (const field of bindingNames) {
                const exemplar = group.flatMap((s) =>
                    s.tree.bindings.filter((b) => b.name === field),
                )[0];
                const entries = group.map((s) => ({
                    values: s.values,
                    code:
                        s.tree.bindings.find((b) => b.name === field)?.value
                            .code ?? defaults[field],
                }));
                if (
                    rootElement &&
                    rootVisible !== "true" &&
                    field === "opacity"
                )
                    for (const entry of entries)
                        entry.code = `${rootVisible} ? (${entry.code}) : 0`;
                const baseCode = appearanceDefault(
                    entries.map((entry) => entry.code),
                );
                if (entries.every((e) => e.code === entries[0].code)) {
                    if (!isDefaultBinding(first.type, field, entries[0].code))
                        lines.push(
                            printLine({
                                ...exemplar,
                                depth: bodyDepth,
                                value: {
                                    ...exemplar.value,
                                    code: entries[0].code,
                                },
                            }),
                        );
                } else {
                    if (!isDefaultBinding(first.type, field, baseCode))
                        lines.push(
                            printLine(
                                binding(
                                    field,
                                    {
                                        ...exemplar.value,
                                        code: baseCode,
                                    },
                                    bodyDepth,
                                ),
                            ),
                        );
                    entries.forEach((entry, i) => {
                        appearances[i].properties[field] = entry.code;
                    });
                }
            }
            lines.push(...appearanceStates(appearances, axes, bodyDepth));
            for (const childKey of orderedKeys) {
                const childGroup = group.flatMap((s, i) => {
                    const tree =
                        s.tree.children[childKeys[i].indexOf(childKey)];
                    return tree ? [{ tree, values: s.values }] : [];
                });
                if (childGroup.length !== group.length) {
                    const condition = selectValues(
                        group.map((s, i) => ({
                            values: s.values,
                            code: childKeys[i].includes(childKey)
                                ? "true"
                                : "false",
                        })),
                        axes,
                        "false",
                    );
                    if (condition.length <= 100) {
                        lines.push(
                            ...emitTree(
                                childGroup,
                                bodyDepth,
                                false,
                                condition === "true"
                                    ? undefined
                                    : `(${condition})`,
                                first.type === "FlexboxLayout",
                            ),
                        );
                    } else {
                        const pred = allocateProperty(
                            `show-${identifier(childGroup[0].tree.origin?.name ?? `child-${++serial}`)}`,
                        );
                        declarations.push(
                            ...declaration(
                                `    private property <bool> ${pred}: `,
                                condition,
                            ),
                        );
                        lines.push(
                            ...emitTree(
                                childGroup,
                                bodyDepth,
                                false,
                                `root.${pred}`,
                                first.type === "FlexboxLayout",
                            ),
                        );
                    }
                } else
                    lines.push(
                        ...emitTree(
                            childGroup,
                            bodyDepth,
                            false,
                            undefined,
                            first.type === "FlexboxLayout",
                        ),
                    );
            }
            if (!rootElement) lines.push(`${indent}}`);
            return lines;
        }
        const body = emitTree(samples, 1, true);
        source.push(
            `export component ${name} inherits Rectangle {`,
            ...declarations,
            ...body,
            ...(behavior?.body ?? []),
            "}",
            "",
        );
        // Occurrences never feed back into the family model.
        const specializations = new Map<string, string>();
        for (const node of [...(occurrences.get(id) ?? [])].sort((a, b) =>
            a.id < b.id ? -1 : 1,
        )) {
            const ref = library.references[node.id];
            const variant = variants.find((v) => v.id === ref.variantId);
            if (!variant) throw Error(`Unknown variant for ${node.id}`);
            const original = requireValue(variantTrees.get(variant.id));
            const actual = rendered(node);
            const bound = applyContract(
                actual,
                d.contract,
                publicNames,
                original,
            );
            const model = applyContract(original, d.contract, publicNames);
            const bindings = axes
                .filter(
                    (a) =>
                        behavior?.axis === a.key ||
                        variant.values[a.key] !== d.axes[a.key].defaultValue,
                )
                .map((a) =>
                    binding(
                        a.property,
                        literal(a.type, optionValue(a, variant.values[a.key])),
                        0,
                    ),
                );
            for (const [key, p] of publicNames) {
                const value = ref.properties?.[key];
                if (value !== undefined && value !== p.property.defaultValue)
                    bindings.push(
                        binding(
                            p.name,
                            literal(p.type, JSON.stringify(value)),
                            0,
                        ),
                    );
                else if (p.type === "string") {
                    // Recover effective text overrides from the same declared property path.
                    function find(a: Element, m: Element): string | undefined {
                        if (
                            d.contract?.bindings[m.origin?.id ?? ""]
                                ?.characters === key
                        )
                            return a.bindings.find((b) => b.name === "text")
                                ?.value.code;
                        for (let i = 0; i < m.children.length; i++)
                            if (
                                a.children[i] &&
                                elementKey(a.children[i]) ===
                                    elementKey(m.children[i])
                            ) {
                                const value = find(
                                    a.children[i],
                                    m.children[i],
                                );
                                if (value !== undefined) return value;
                            }
                    }
                    const value = find(actual, original);
                    if (
                        value !== undefined &&
                        value !== JSON.stringify(p.property.defaultValue)
                    )
                        bindings.push(
                            binding(p.name, literal(p.type, value), 0),
                        );
                }
            }
            const visibleTree = (tree: Element): Element => ({
                ...tree,
                children: tree.children
                    .filter((child) => {
                        const p = [...publicNames.entries()].find(
                            ([, p]) => child.condition === `root.${p.name}`,
                        );
                        return (
                            !p ||
                            (ref.properties?.[p[0]] ??
                                p[1].property.defaultValue) !== false
                        );
                    })
                    .map(visibleTree),
            });
            if (
                structuralSignature(visibleTree(bound)) ===
                structuralSignature(visibleTree(model))
            )
                uses.set(node, { name, bindings, templateBindings: bindings });
            else {
                const sig = structuralSignature(actual);
                let specialized = specializations.get(sig);
                if (!specialized) {
                    specialized = allocateType(`${name}Override`);
                    specializations.set(sig, specialized);
                    source.push(
                        `component ${specialized} inherits Rectangle {`,
                        `    preferred-width: ${node.width}px;`,
                        `    preferred-height: ${node.height}px;`,
                        ...actual.bindings.map((b) =>
                            printLine({ ...b, depth: 1 }),
                        ),
                        ...actual.children.flatMap((c) =>
                            treeLines(c, 1).map(printLine),
                        ),
                        "}",
                        "",
                    );
                    warnings.push({
                        severity: "warning",
                        code: "COMPONENT_INSTANCE_SPECIALIZED",
                        nodeId: node.id,
                        nodeName: node.name,
                        message: `${node.name}: overrides outside the authored contract use private ${specialized}`,
                    });
                }
                uses.set(node, {
                    name: specialized,
                    bindings: [],
                    templateBindings: [],
                });
            }
        }
    }
    warnings.push(...tokens.warnings);
    return {
        source: [...assets.source(), ...tokens.source, ...source],
        uses,
        warnings,
    };
}

/** Intern images in the typed tree before factoring variants. Keep PNG payloads
 * outside the component body, and preserve a single self-contained source file. */
function componentAssets(globalName: string) {
    const images = new Map<string, string>();
    const allocate = nameAllocator();
    function apply(tree: Element): Element {
        return {
            ...tree,
            bindings: tree.bindings.map((b) => {
                if (b.value.type !== "image" || b.value.kind !== "literal")
                    return b;
                let name = images.get(b.value.code);
                if (!name) {
                    name = allocate(identifier(tree.origin?.name || "icon"));
                    images.set(b.value.code, name);
                }
                return {
                    ...b,
                    value: reference("image", `${globalName}.${name}`),
                };
            }),
            children: tree.children.map(apply),
        };
    }
    return {
        apply,
        source: () =>
            images.size
                ? [
                      `global ${globalName} {`,
                      ...[...images].map(
                          ([code, name]) =>
                              `    out property <image> ${name}: ${code};`,
                      ),
                      "}",
                      "",
                  ]
                : [],
    };
}

/** Defaults verified against Slint builtins. Do not erase layout bindings here:
 * e.g. FlexboxLayout wraps by default, and explicit stretch affects intrinsic size. */
const rectangleBindingDefaults: Record<string, string> = {
    background: "transparent",
    "border-color": "transparent",
    "border-width": "0px",
    "border-radius": "0px",
    clip: "false",
    opacity: "1",
};
function isDefaultBinding(
    elementType: string,
    field: string,
    code: string,
): boolean {
    return (
        elementType === "Rectangle" && rectangleBindingDefaults[field] === code
    );
}
