// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Element } from "./slint-ir";

const role = (tree: Element) => {
    const text = tree.bindings.find(
        (binding) =>
            binding.name === "text" && binding.value.kind === "reference",
    );
    return `${tree.type}:${text ? `property:${text.value.code}` : (tree.role ?? tree.origin?.name ?? "helper")}`;
};

/** Authored names establish roles before geometry is lowered into helpers.
 * Same-type siblings with different names are not interchangeable content. */
export function childRoles(trees: Element[]): string[][] {
    return trees.map((tree) => tree.children.map((child) => role(child)));
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
    role?: string;
    bindings: readonly unknown[];
    children: StructuralSignature[];
};

export function structuralSignature(tree: Element): string {
    const build = (element: Element): StructuralSignature => ({
        type: element.type,
        condition: element.condition,
        role: element.role,
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
        element.role,
        element.bindings.map((binding) => [
            binding.name,
            binding.value.type,
            binding.value.type === "opaque" ? binding.value.code : null,
        ]),
        element.children.map(build),
    ];
    return JSON.stringify(build(tree));
}
