// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SourceNode } from "./source";

/** Serialized variable metadata needed by native code snippets. */
export type CodegenVariable = {
    field: string;
    name?: string;
    collection?: string;
    modes?: number;
    type?: string;
};

export function codegenAliases(
    root: SourceNode,
): { field: string; id: string }[] {
    const result: { field: string; id: string }[] = [];
    function alias(field: string, value: unknown) {
        if (
            value &&
            typeof value === "object" &&
            "type" in value &&
            value.type === "VARIABLE_ALIAS" &&
            "id" in value &&
            typeof value.id === "string"
        )
            result.push({ field, id: value.id });
    }
    const bound = root.properties.boundVariables;
    if (bound && typeof bound === "object" && !Array.isArray(bound))
        for (const [field, value] of Object.entries(bound)) alias(field, value);
    for (const field of ["fills", "strokes"] as const) {
        const paints = root.properties[field];
        if (!Array.isArray(paints)) continue;
        const visible = paints.filter(
            (p) =>
                p &&
                typeof p === "object" &&
                !Array.isArray(p) &&
                p.visible !== false,
        );
        if (visible.length !== 1) continue;
        const paint = visible[0];
        if (
            !paint ||
            typeof paint !== "object" ||
            Array.isArray(paint) ||
            paint.type !== "SOLID"
        )
            continue;
        const bindings = paint.boundVariables;
        if (
            bindings &&
            typeof bindings === "object" &&
            !Array.isArray(bindings)
        )
            alias(
                paint.opacity === undefined || paint.opacity === 1
                    ? field
                    : `${field}.opacity`,
                bindings.color,
            );
    }
    return result;
}

// Preserve the names used by the published inspector's variable exporter.
export function codegenVariablePath(
    variable: CodegenVariable,
): string | undefined {
    if (
        variable.name === undefined ||
        variable.collection === undefined ||
        !variable.modes
    )
        return;
    let collection = variable.collection.replace(/^\./, "");
    if (!collection.trim()) collection = "property";
    collection = collection
        .replace(/&/g, "and")
        .replace(/,\s*|\+|:|\/|—/g, "-")
        .replace(/([a-z])([A-Z])/g, "$1-$2")
        .replace(/\s+/g, "-")
        .replace(/--+/g, "-")
        .toLowerCase();
    const parts = variable.name.split("/").map((part) => {
        let name = part
            .replace(/^\./, "")
            .replace(/^[_\-.()&]+/, "")
            .replace(/&/g, "and")
            .replace(/[^a-zA-Z0-9-]/g, "-")
            .replace(/-+/g, "-")
            .replace(/^-+|-+$/g, "");
        if (!name) name = "property";
        return /^\d/.test(name) ? `_${name}` : name.toLowerCase();
    });
    const path = [
        collection,
        ...(variable.modes > 1 ? ["current"] : []),
        ...parts,
    ];
    // Invalid legacy identifiers must not inject malformed Slint expressions.
    if (path.some((part) => !/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(part))) return;
    return path.join(".");
}
