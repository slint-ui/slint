// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Element } from "./slint-ir";

const role = (tree: Element) => `${tree.type}:${tree.origin?.name ?? "helper"}`;
function shape(tree: Element): string {
    return elementShape(tree);
}

/** A unique element type gives unambiguous sibling correspondence even when
 * authored names, optional decorations, or binding schemas differ by variant.
 * Otherwise prefer authored names, then uniquely matching schemas. Repeated
 * ambiguous siblings are deliberately left for the exact-tree fallback. */
export function childRoles(trees: Element[]): string[][] {
    const shapes = trees.map((tree) => tree.children.map(shape));
    return trees.map((tree, i) =>
        tree.children.map((child, j) => {
            if (
                trees.every(
                    (t) =>
                        t.children.filter((c) => c.type === child.type)
                            .length <= 1,
                )
            )
                return `type:${child.type}`;
            const named = role(child);
            if (
                trees.filter((t) => t.children.some((c) => role(c) === named))
                    .length > 1
            )
                return named;
            const schema = shapes[i][j];
            if (
                shapes.every(
                    (list) => list.filter((s) => s === schema).length <= 1,
                )
            )
                return `shape:${schema}`;
            return named;
        }),
    );
}

/** Combine partial sibling orders without treating the first variant as a
 * complete order. Conflicting orders or repeated roles require a fallback. */
export function childOrder(lists: string[][]): string[] | undefined {
    if (lists.some((list) => new Set(list).size !== list.length)) return;
    const keys = [...new Set(lists.flat())];
    const edges = new Map(keys.map((key) => [key, new Set<string>()]));
    for (const list of lists)
        for (let i = 1; i < list.length; i++)
            edges.get(list[i - 1])?.add(list[i]);
    const result: string[] = [];
    const remaining = new Set(keys);
    while (remaining.size) {
        const next = keys.find(
            (key) =>
                remaining.has(key) &&
                ![...remaining].some((from) => edges.get(from)?.has(key)),
        );
        if (next === undefined) return;
        result.push(next);
        remaining.delete(next);
    }
    return result;
}

type StructuralSignature = {
    type: string;
    condition?: string;
    bindings: readonly unknown[];
    children: StructuralSignature[];
};

export function structuralSignature(tree: Element): string {
    const build = (element: Element): StructuralSignature => ({
        type: element.type,
        condition: element.condition,
        bindings: element.bindings.map((binding) => [
            binding.name,
            binding.value,
        ]),
        children: element.children.map(build),
    });
    return JSON.stringify(build(tree));
}

export function elementShape(tree: Element): string {
    const build = (element: Element): unknown[] => [
        element.type,
        element.condition,
        element.bindings.map((binding) => [
            binding.name,
            binding.value.type,
            binding.value.kind === "raw" ? binding.value.code : null,
        ]),
        element.children.map(build),
    ];
    return JSON.stringify(build(tree));
}
