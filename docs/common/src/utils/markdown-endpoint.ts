// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";

/** Minimal structural shape of an `astro:content` collection entry. */
export interface MarkdownDocEntry {
    /** Collection id; the root index page has an empty id. */
    id: string;
    /** Frontmatter data (title, description, …). */
    data: Record<string, unknown>;
    /** Raw markdown body. */
    body?: string;
    /**
     * Source file path relative to the project root (Astro's
     * `entry.filePath`). Needed to resolve relative `?raw` imports.
     */
    filePath?: string;
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
    /**
     * Absolute path of the Astro project root. When set, `?raw` imports and
     * the `<Code>` components consuming them are inlined as fenced code
     * blocks. Sites whose pages don't import code from files omit this.
     */
    projectRoot?: string;
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
    if (options.projectRoot) {
        body = inlineRawCodeImports(body, options.projectRoot, entry.filePath);
    }
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

// Pages include code examples via Vite raw imports —
// `import name from "…?raw"` hands the file's text to a `<Code>` component at
// build time. The served markdown is the unprocessed MDX, so without help the
// reader sees the import line where the HTML page shows code. Inline the file
// content as a fenced code block and drop the consumed import. Components
// whose source can't be resolved are left as-is so the regular build checks
// surface the problem.
const RAW_IMPORT_RE =
    /^import\s+(\w+)\s+from\s+["']([^"']+)\?raw["'];?[ \t]*\n?/gm;
const CODE_COMPONENT_RE = /<Code\s([^>]*?)\/>/g;
// Mirrors `extractLines()` from utils.ts: a 1-based inclusive line range.
const EXTRACT_LINES_RE =
    /^\s*extractLines\(\s*(\w+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*$/;

function inlineRawCodeImports(
    body: string,
    projectRoot: string,
    entryFilePath?: string,
): string {
    // The imported files: binding name -> file content.
    const sources = new Map<string, string>();
    for (const [, name, importPath] of body.matchAll(RAW_IMPORT_RE)) {
        // Project-absolute imports (`/src/…`) resolve against the project
        // root; relative ones against the page's own directory.
        const resolved = importPath.startsWith("/")
            ? join(projectRoot, importPath)
            : entryFilePath
              ? join(projectRoot, dirname(entryFilePath), importPath)
              : undefined;
        try {
            if (resolved) {
                sources.set(name, readFileSync(resolved, "utf-8"));
            }
        } catch {
            // Unreadable file: keep the import and its <Code> usage verbatim.
        }
    }
    if (sources.size === 0) {
        return body;
    }

    const inlined = new Set<string>();
    const out = body.replace(CODE_COMPONENT_RE, (whole, attrs: string) => {
        const codeExpr = /\bcode=\{([^}]*)\}/.exec(attrs)?.[1] ?? "";
        const sliced = EXTRACT_LINES_RE.exec(codeExpr);
        const name = sliced?.[1] ?? codeExpr.trim();
        let content = sources.get(name);
        if (content === undefined) {
            return whole;
        }
        if (sliced) {
            content = content
                .split("\n")
                .slice(Number(sliced[2]) - 1, Number(sliced[3]))
                .join("\n");
        }
        inlined.add(name);
        const lang = /\blang="([^"]*)"/.exec(attrs)?.[1] ?? "";
        const title = /\btitle="([^"]*)"/.exec(attrs)?.[1];
        // A fence must be longer than any backtick run in the content.
        let fence = "```";
        while (content.includes(fence)) {
            fence += "`";
        }
        const info = lang + (title ? ` title="${title}"` : "");
        return `${fence}${info}\n${content.replace(/\n+$/, "")}\n${fence}`;
    });

    // Drop the import lines whose code is now inlined.
    return out.replace(RAW_IMPORT_RE, (whole, name) =>
        inlined.has(name) ? "" : whole,
    );
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
