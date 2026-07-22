// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore slugified

/** Starlight collects pages under `src/content/docs`; the generated API lives under `api/`. */
export const API_ROOT = "api";

/**
 * A filesystem/url-safe fragment: lowercased, punctuation collapsed to `-`.
 * Underscores are kept (like {@link memberAnchorBase}) so the slug mirrors the
 * C++ identifier, e.g. `update_all_translations` not `update-all-translations`.
 */
function slugify(text: string): string {
    return text
        .replace(/::/g, "-")
        .replace(/[^a-zA-Z0-9_]+/g, "-")
        .replace(/-+/g, "-")
        .replace(/^-|-$/g, "")
        .toLowerCase();
}

/** Turn a `::`-separated namespace name into `/`-joined, slugified path segments. */
function namespacePath(name: string): string {
    return name.split("::").map(slugify).filter(Boolean).join("/");
}

/**
 * Page slug (relative to the docs content root) for a compound. The reference
 * is organized by namespace rather than by kind: namespaces become directories
 * and every type (class, struct, …) lives under its enclosing namespace, so the
 * class/struct distinction never appears in the path. Examples:
 * `slint` → `api/slint`, `slint::Color` → `api/slint/color`,
 * `slint::platform::Platform::Task` → `api/slint/platform/platform-task`.
 *
 * `namespaceNames` is the set of all namespace names; the enclosing namespace of
 * a type is the longest one that prefixes its qualified name (the remainder,
 * including any nested-class path, collapses into one slug segment).
 */
export function compoundSlug(
    kind: string,
    qualifiedName: string,
    namespaceNames: Iterable<string> = [],
): string {
    if (kind === "namespace") {
        return `${API_ROOT}/${namespacePath(qualifiedName)}`;
    }
    let owner = "";
    for (const ns of namespaceNames) {
        if (qualifiedName.startsWith(`${ns}::`) && ns.length > owner.length) {
            owner = ns;
        }
    }
    const leaf = owner ? qualifiedName.slice(owner.length + 2) : qualifiedName;
    const dir = owner ? `${namespacePath(owner)}/` : "";
    return `${API_ROOT}/${dir}${slugify(leaf)}`;
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
 * e.g. `api/slint`). Relative links resolve correctly whether the
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
