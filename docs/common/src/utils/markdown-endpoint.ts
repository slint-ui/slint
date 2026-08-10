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
    /**
     * Fold the C++ API reference's `api-signature` `<pre>` HTML into ```cpp
     * fences and drop the empty heading anchors. Only the doxygen-generated
     * pages carry that HTML (and the fence is C++), so other sites omit this.
     */
    apiSignatures?: boolean;
}

/**
 * Map a `docs` collection into the `{ params, props }` array expected by an
 * Astro `getStaticPaths`. The root index (empty id) becomes `index.md`.
 */
export function markdownStaticPaths(entries: MarkdownDocEntry[]) {
    return entries.map((entry) => ({
        params: { slug: `${markdownRouteContentPath(entry.id)}.md` },
        props: { entry },
    }));
}

function markdownRouteContentPath(id: string): string {
    if (id === "") {
        return "index";
    }
    // cspell:ignore mkdn mdwn
    return id.replace(/\.(?:md|mdx|markdown|mdown|mkdn|mkd|mdwn)$/i, "");
}

/** Build the `text/markdown` Response for a single doc entry. */
export function renderMarkdownResponse(
    entry: MarkdownDocEntry,
    options: MarkdownEndpointOptions = {},
): Response {
    const data = entry.data;
    let body = entry.body ?? "";
    if (options.apiSignatures) {
        body = simplifyApiReferenceHtml(body);
    }
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

// The C++ pages render signatures as `<pre class="api-signature">` HTML and
// give member headings an `<a id>` anchor — both noise in the plain markdown an
// agent reads. Fold signatures into ```cpp fences, drop the anchors and the
// `<small>` markers (keeping their text). A no-op for pages without that HTML.
const API_SIGNATURE_RE =
    /<pre\b[^>]*class="[^"]*\bapi-signature\b[^"]*"[^>]*>([\s\S]*?)<\/pre>/g;
// cSpell:ignore apos
const NAMED_ENTITY: Record<string, string> = {
    lt: "<",
    gt: ">",
    quot: '"',
    apos: "'",
    amp: "&",
};

// Decode the entities in the signature markup. Shiki escapes `<` as the hex
// `&#x3C;`, so numeric refs are handled too, not just named ones. `&amp;` is
// decoded last so an escaped `&amp;lt;` survives as the literal `&lt;`.
function decodeHtmlEntities(s: string): string {
    return s
        .replace(
            /&#x([0-9a-fA-F]+);|&#(\d+);|&(lt|gt|quot|apos);/g,
            (_, hex: string, dec: string, name: string) => {
                if (hex !== undefined) {
                    return String.fromCodePoint(Number.parseInt(hex, 16));
                }
                if (dec !== undefined) {
                    return String.fromCodePoint(Number.parseInt(dec, 10));
                }
                return NAMED_ENTITY[name];
            },
        )
        .replace(/&amp;/g, "&");
}

function simplifyApiReferenceHtml(body: string): string {
    return (
        body
            .replace(API_SIGNATURE_RE, (_whole, inner: string) => {
                const code = inner
                    // Keep only the `<code>…</code>` payload.
                    .replace(/^[\s\S]*?<code[^>]*>/, "")
                    .replace(/<\/code>[\s\S]*$/, "")
                    // Shiki wraps each source line in its own span.
                    .replace(/<span class="line">/g, "\n")
                    .replace(/<[^>]+>/g, "");
                const text = decodeHtmlEntities(code)
                    .replace(/^\n+/, "")
                    .replace(/\n+$/, "");
                return `\`\`\`cpp\n${text}\n\`\`\``;
            })
            // Empty heading anchors (`### <a id="name"></a> \`name\``) and the
            // `<small>(virtual)</small>` markers around member headings.
            .replace(/<a id="[^"]*"><\/a>\s?/g, "")
            .replace(/<\/?small>/g, "")
    );
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
// content as a fenced code block and drop the consumed import. Anything that
// doesn't parse fails the build: a silent pass-through would let a syntax
// change (a new Astro major, a new authoring pattern) quietly degrade the
// served markdown again.
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
    const page = entryFilePath ?? "<unknown page>";
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
        if (resolved === undefined) {
            throw new Error(
                `${page}: cannot resolve the relative import "${importPath}?raw" without the entry's filePath`,
            );
        }
        try {
            sources.set(name, readFileSync(resolved, "utf-8"));
        } catch {
            // Astro itself resolves the import during the build, so a read
            // failure here means this endpoint resolved the path differently.
            throw new Error(
                `${page}: cannot read "${importPath}?raw" (resolved to ${resolved}) — fix the path resolution in the markdown endpoint`,
            );
        }
    }

    const inlined = new Set<string>();
    const out = body.replace(CODE_COMPONENT_RE, (whole, attrs: string) => {
        const codeExpr = /\bcode=\{([^}]*)\}/.exec(attrs)?.[1] ?? "";
        const sliced = EXTRACT_LINES_RE.exec(codeExpr);
        const name = sliced?.[1] ?? codeExpr.trim();
        let content = sources.get(name);
        if (content === undefined) {
            throw new Error(
                `${page}: \`${whole}\` does not reference a ?raw import — teach the markdown endpoint this syntax`,
            );
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

    for (const name of sources.keys()) {
        if (!inlined.has(name)) {
            throw new Error(
                `${page}: the ?raw import "${name}" feeds no <Code …/> the markdown endpoint recognizes — teach it the new syntax`,
            );
        }
    }
    // Every import is consumed at this point; drop the import lines.
    return out.replace(RAW_IMPORT_RE, "");
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
