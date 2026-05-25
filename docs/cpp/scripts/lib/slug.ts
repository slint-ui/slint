// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Starlight collects pages under `src/content/docs`; the generated API lives under `api/`. */
export const API_ROOT = "api";

const KIND_DIR: Record<string, string> = {
    class: "classes",
    struct: "structs",
    union: "unions",
    interface: "interfaces",
    namespace: "namespaces",
    group: "groups",
    concept: "concepts",
};

/** A filesystem/url-safe fragment, lowercased, with `::` and other separators collapsed to `-`. */
function slugify(text: string): string {
    return text
        .replace(/::/g, "-")
        .replace(/[^a-zA-Z0-9]+/g, "-")
        .replace(/-+/g, "-")
        .replace(/^-|-$/g, "")
        .toLowerCase();
}

/** Page slug (relative to the docs content root) for a compound, e.g. `api/classes/slint-color`. */
export function compoundSlug(kind: string, qualifiedName: string): string {
    const dir = KIND_DIR[kind] ?? "other";
    return `${API_ROOT}/${dir}/${slugify(qualifiedName)}`;
}

/**
 * A stable in-page anchor for a member, disambiguating overloads with a numeric
 * suffix. Underscores are kept so the anchor mirrors the C++ identifier; other
 * symbols (e.g. in `operator==`) collapse to `-`.
 */
export function memberAnchor(name: string, overloadIndex: number): string {
    const base =
        name
            .replace(/[^a-zA-Z0-9_]+/g, "-")
            .replace(/-+/g, "-")
            .replace(/^-|-$/g, "")
            .toLowerCase() || "member";
    return overloadIndex === 0 ? base : `${base}-${overloadIndex + 1}`;
}
