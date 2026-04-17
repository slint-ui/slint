// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Serves a plain-markdown sibling for every doc page so AI agents can fetch
// docs without paying the HTML/JS overhead. Example:
//   /docs/guide/language/coding/properties/      -> rendered HTML page
//   /docs/guide/language/coding/properties.md    -> this endpoint, raw markdown

import type { APIRoute, GetStaticPaths } from "astro";
import { getCollection, type CollectionEntry } from "astro:content";

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
    const body = entry.body ?? "";

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
