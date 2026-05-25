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
 * Base anchor for a member. Underscores are kept so the anchor mirrors the C++
 * identifier; other symbols (e.g. in `operator==`) collapse to `-`. Callers
 * disambiguate collisions (overloads, or distinct operators that slug the same)
 * via {@link disambiguateAnchor}.
 */
export function memberAnchorBase(name: string): string {
    return (
        name
            .replace(/[^a-zA-Z0-9_]+/g, "-")
            .replace(/-+/g, "-")
            .replace(/^-|-$/g, "")
            .toLowerCase() || "member"
    );
}

/** Append a numeric suffix for the Nth (0-based) use of the same base anchor. */
export function disambiguateAnchor(base: string, occurrence: number): string {
    return occurrence === 0 ? base : `${base}-${occurrence + 1}`;
}

/**
 * A relative URL from one page slug to another (both relative to the docs root,
 * e.g. `api/namespaces/slint`). Relative links resolve correctly whether the
 * site is served at `/` or under a base like `/master/docs/cpp/`, unlike
 * root-absolute links which Astro does not rewrite with the base. Pages use
 * `trailingSlash: "always"`, so each slug is served as its own directory.
 */
export function relativeUrl(fromSlug: string, toSlug: string): string {
    const from =
        fromSlug === "" || fromSlug === "index" ? [] : fromSlug.split("/");
    const to = toSlug.split("/");
    let common = 0;
    while (
        common < from.length &&
        common < to.length &&
        from[common] === to[common]
    ) {
        common++;
    }
    const ups = "../".repeat(from.length - common);
    const downs = to.slice(common).join("/");
    const rel = ups + (downs ? `${downs}/` : "");
    return rel === "" ? "./" : rel;
}
