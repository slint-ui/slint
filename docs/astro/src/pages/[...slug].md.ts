// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Serves a plain-markdown sibling for every doc page so AI agents can fetch
// docs without paying the HTML/JS overhead. Example:
//   /docs/guide/language/coding/properties/      -> rendered HTML page
//   /docs/guide/language/coding/properties.md    -> this endpoint, raw markdown

import type { APIRoute, GetStaticPaths } from "astro";
import { getCollection, type CollectionEntry } from "astro:content";
import { linkMap } from "@slint/common-files/src/utils/utils";
import { BASE_PATH } from "@slint/common-files/src/utils/site-config";

export const getStaticPaths: GetStaticPaths = async () => {
    const entries = await getCollection("docs");
    return entries.map((entry) => ({
        // Empty id (the root index.mdx) becomes /docs/index.md
        params: { slug: entry.id === "" ? "index" : entry.id },
        props: { entry },
    }));
};

type Props = { entry: CollectionEntry<"docs"> };

export const GET: APIRoute<Props> = ({ props }) => {
    const { entry } = props;
    const data = entry.data as Record<string, unknown>;
    const body = resolveLinkComponents(entry.body ?? "");

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
};

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

function resolveLinkComponents(body: string): string {
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
        return `[${label}](${BASE_PATH}${toMarkdownHref(linkMap[type].href)})`;
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
