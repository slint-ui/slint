// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Minimal structural shape of an `astro:content` collection entry. */
export interface MarkdownDocEntry {
    /** Collection id; the root index page has an empty id. */
    id: string;
    /** Frontmatter data (title, description, …). */
    data: Record<string, unknown>;
    /** Raw markdown body. */
    body?: string;
}

/** A `linkMap` entry: the in-prose `<Link>` component resolves a `type` to an href. */
export interface MarkdownLinkTarget {
    href: string;
}

export interface MarkdownEndpointOptions {
    /**
     * Base path prefixed onto each resolved `<Link>` href (e.g. `/docs/`). Only
     * used when `linkMap` is provided.
     */
    basePath?: string;
    /**
     * Optional map for rewriting the astro-specific `<Link type=… label=…/>`
     * prose component into real markdown links. Sites without that component
     * (the generated C++/Python/Node API references) omit this.
     */
    linkMap?: Record<string, MarkdownLinkTarget>;
}

/**
 * Map a `docs` collection into the `{ params, props }` array expected by an
 * Astro `getStaticPaths`. The root index (empty id) becomes `index.md`.
 */
export function markdownStaticPaths(entries: MarkdownDocEntry[]) {
    return entries.map((entry) => ({
        params: { slug: entry.id === "" ? "index" : entry.id },
        props: { entry },
    }));
}

/** Build the `text/markdown` Response for a single doc entry. */
export function renderMarkdownResponse(
    entry: MarkdownDocEntry,
    options: MarkdownEndpointOptions = {},
): Response {
    const data = entry.data;
    let body = entry.body ?? "";
    if (options.linkMap) {
        body = resolveLinkComponents(
            body,
            options.linkMap,
            options.basePath ?? "",
        );
    }

    const fm: string[] = ["---"];
    if (typeof data.title === "string") {
        fm.push(`title: ${quote(data.title)}`);
    }
    if (typeof data.description === "string") {
        fm.push(`description: ${quote(data.description)}`);
    }
    fm.push("---", "");

    return new Response(fm.join("\n") + body, {
        headers: {
            "Content-Type": "text/markdown; charset=utf-8",
            // Lets a future edge function vary the HTML route on Accept
            // without poisoning shared caches.
            Vary: "Accept",
        },
    });
}

// YAML-safe double-quoting for single-line scalar values.
function quote(s: string): string {
    return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

// Replace `<Link type="X" label="Y" />` (the in-prose linking component used
// across the docs) with a real markdown link. The resolved URL points at the
// target page's .md sibling so an agent can chain markdown fetches without
// having to rewrite URLs itself. Unknown link types are left as-is so the
// build still surfaces them via existing checks.
const LINK_RE = /<Link\b([^/>]*)\/>/g;
const ATTR_RE = /(\w+)\s*=\s*"([^"]*)"/g;

function resolveLinkComponents(
    body: string,
    linkMap: Record<string, MarkdownLinkTarget>,
    basePath: string,
): string {
    return body.replace(LINK_RE, (whole, attrs: string) => {
        const parsed: Record<string, string> = {};
        for (const m of attrs.matchAll(ATTR_RE)) {
            parsed[m[1]] = m[2];
        }

        const type = parsed.type;
        if (!type || !(type in linkMap)) {
            return whole;
        }

        const label = parsed.label ?? type;
        return `[${label}](${basePath}${toMarkdownHref(linkMap[type].href)})`;
    });
}

// Convert an HTML page href like "reference/common/#anchor" into the
// corresponding .md sibling: "reference/common.md#anchor".
function toMarkdownHref(href: string): string {
    const hashIdx = href.indexOf("#");
    const path = hashIdx >= 0 ? href.slice(0, hashIdx) : href;
    const hash = hashIdx >= 0 ? href.slice(hashIdx) : "";
    const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
    return `${trimmed}.md${hash}`;
}
