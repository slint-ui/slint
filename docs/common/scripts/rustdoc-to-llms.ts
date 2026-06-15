// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Generate `llms.txt` (curated index) and `llms-full.txt` (full text) for the
// Slint *Rust* API from the rustdoc HTML output.
//
// Why HTML and not rustdoc JSON: the `slint` crate is a facade that re-exports
// most of its public API from other crates (`i-slint-core`, the renderers, the
// backend selector, ...) via plain `pub use` — including glob re-exports such as
// `pub use i_slint_core::api::*`. rustdoc JSON does NOT carry cross-crate
// re-exported items in its `index` (even with `#[doc(inline)]`), so JSON of the
// `slint` crate alone would be almost empty. The rustdoc HTML, by contrast,
// inlines every re-export under the correct `slint::` path (this is exactly what
// docs.slint.dev/.../docs/rust/ already serves), so we convert that instead.
//
// This is coupled to rustdoc's generated HTML structure (`#main-content`,
// `.docblock`, `pre.item-decl`, `section.method > h4.code-header`, and the
// `#*-implementations[-list]` sections). If a future rustdoc changes those, the
// selectors below need updating.
//
// Usage:
//   node --experimental-strip-types rustdoc-to-llms.ts \
//     --doc-root <dir> --crates slint,slint_interpreter,slint_build [--out <dir>]

import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import { join, relative } from "node:path";
import { pathToFileURL } from "node:url";
import { parse } from "node-html-parser";
import type { HTMLElement, Node } from "node-html-parser";

type Kind =
    | "Module"
    | "Struct"
    | "Enum"
    | "Trait"
    | "Function"
    | "Macro"
    | "Type Alias"
    | "Constant"
    | "Static"
    | "Union"
    | "Primitive"
    | "Derive Macro"
    | "Attribute Macro"
    | "Trait Alias";

interface Entry {
    crate: string;
    path: string; // e.g. slint::platform::WindowEvent
    kind: Kind;
    url: string; // e.g. slint/platform/enum.WindowEvent.html (relative to doc-root)
    summary: string;
    markdown: string;
    sortKey: string;
}

// rustdoc file-name prefix -> kind label.
const FILE_KIND: Record<string, Kind> = {
    struct: "Struct",
    enum: "Enum",
    trait: "Trait",
    fn: "Function",
    macro: "Macro",
    type: "Type Alias",
    constant: "Constant",
    static: "Static",
    union: "Union",
    primitive: "Primitive",
    derive: "Derive Macro",
    attr: "Attribute Macro",
    traitalias: "Trait Alias",
};

// Subtrees inside #main-content that are pure noise for an LLM corpus: the
// auto/blanket trait-impl lists (Send/Sync/From<T>/Any/...), the long list of
// implementors, and the in-page table of contents / module nav.
const REMOVE_IDS = [
    "synthetic-implementations",
    "synthetic-implementations-list",
    "blanket-implementations",
    "blanket-implementations-list",
    "trait-implementations",
    "trait-implementations-list",
    "implementors",
    "implementors-list",
    "rustdoc-toc",
    "rustdoc-modnav",
];

const ENTITIES: Record<string, string> = {
    amp: "&",
    lt: "<",
    gt: ">",
    quot: '"',
    apos: "'",
    nbsp: " ",
};

export function decode(s: string): string {
    return s.replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/g, (m, e: string) => {
        if (e in ENTITIES) {
            return ENTITIES[e];
        }
        if (e[0] === "#") {
            const code =
                e[1] === "x" || e[1] === "X"
                    ? Number.parseInt(e.slice(2), 16)
                    : Number.parseInt(e.slice(1), 10);
            return Number.isFinite(code) ? String.fromCodePoint(code) : m;
        }
        return m;
    });
}

function isElement(n: Node): n is HTMLElement {
    return (n as { nodeType?: number }).nodeType === 1;
}

function tagOf(el: HTMLElement): string {
    return (el.rawTagName || "").toUpperCase();
}

// node-html-parser keeps <pre> content as a single raw-text node, so its
// innerHTML is the raw markup (highlight <span>s, intra-doc <a>s, and generics
// as &lt;…&gt; entities). Strip the real tags first, *then* decode entities —
// doing it the other way round would turn `Weak&lt;Self&gt;` into `Weak<Self>`
// and the tag-stripper would eat the `<Self>`.
export function preText(el: HTMLElement): string {
    return decode(el.innerHTML.replace(/<[^>]+>/g, ""));
}

// Convert a rustdoc HTML fragment to Markdown. `pre` keeps whitespace verbatim.
export function render(node: Node, pre = false): string {
    if ((node as { nodeType?: number }).nodeType === 3) {
        const t = decode((node as { rawText?: string }).rawText ?? "");
        return pre ? t : t.replace(/\s+/g, " ");
    }
    if (!isElement(node)) {
        return "";
    }
    const el = node;
    const tag = tagOf(el);
    const cls = el.getAttribute("class") || "";
    if (tag === "SCRIPT" || tag === "STYLE" || tag === "NAV") {
        return "";
    }
    // rustdoc renders impl headers and method signatures inside
    // <h3|h4 class="code-header"> — emit those as Rust code blocks.
    if (/^H[1-6]$/.test(tag) && cls.includes("code-header")) {
        return `\n\n\`\`\`rust\n${el.text.trim()}\n\`\`\`\n\n`;
    }
    const kids = (): string =>
        el.childNodes.map((c) => render(c, pre)).join("");
    switch (tag) {
        case "H1":
            return `\n\n# ${kids().trim()}\n\n`;
        case "H2":
            return `\n\n## ${kids().trim()}\n\n`;
        case "H3":
            return `\n\n### ${kids().trim()}\n\n`;
        case "H4":
            return `\n\n#### ${kids().trim()}\n\n`;
        case "H5":
        case "H6":
            return `\n\n##### ${kids().trim()}\n\n`;
        case "P":
            return `\n\n${kids().trim()}\n\n`;
        case "BR":
            return "  \n";
        case "HR":
            return "\n\n---\n\n";
        case "PRE": {
            const lang = cls.includes("rust") ? "rust" : "";
            const code = preText(el).replace(/\s+$/, "");
            return `\n\n\`\`\`${lang}\n${code}\n\`\`\`\n\n`;
        }
        case "CODE":
            return `\`${el.text.replace(/`/g, "")}\``;
        case "A": {
            const href = el.getAttribute("href");
            const txt = kids().trim() || el.text.trim();
            if (!txt) {
                return "";
            }
            return href && !href.startsWith("#") ? `[${txt}](${href})` : txt;
        }
        case "STRONG":
        case "B":
            return `**${kids().trim()}**`;
        case "EM":
        case "I":
        case "VAR":
            return `*${kids().trim()}*`;
        case "UL": {
            const items = el.childNodes.filter(
                (c) => isElement(c) && tagOf(c) === "LI",
            ) as HTMLElement[];
            if (!items.length) {
                return "";
            }
            return `\n\n${items
                .map((li) => `- ${render(li).trim().replace(/\n/g, "\n  ")}`)
                .join("\n")}\n\n`;
        }
        case "OL": {
            const items = el.childNodes.filter(
                (c) => isElement(c) && tagOf(c) === "LI",
            ) as HTMLElement[];
            if (!items.length) {
                return "";
            }
            return `\n\n${items
                .map(
                    (li, i) =>
                        `${i + 1}. ${render(li).trim().replace(/\n/g, "\n   ")}`,
                )
                .join("\n")}\n\n`;
        }
        case "LI":
            return kids();
        case "BLOCKQUOTE":
            return `\n\n${kids()
                .trim()
                .split("\n")
                .map((l) => `> ${l}`)
                .join("\n")}\n\n`;
        default:
            // div, span, section, details, summary, header, article, dl, ...
            return kids();
    }
}

function cleanup(md: string): string {
    return `${md
        .replace(/^Expand description\s*$/gm, "")
        .replace(/[ \t]+\n/g, "\n")
        .replace(/\n{3,}/g, "\n\n")
        .trim()}\n`;
}

// Push every Markdown heading one level deeper so a page's content nests under
// the `## <Kind> <path>` title we synthesize for it.
function demoteHeadings(md: string): string {
    return md.replace(/^(#{1,5}) /gm, "#$1 ");
}

export function firstSentence(text: string): string {
    const t = decode(text).replace(/\s+/g, " ").trim();
    if (!t) {
        return "";
    }
    const m = t.match(/^(.*?\.)(?:\s|$)/);
    return (m ? m[1] : t).slice(0, 200);
}

function prune(main: HTMLElement): void {
    for (const id of REMOVE_IDS) {
        main.querySelector(`#${id}`)?.remove();
    }
    for (const sel of [
        "a.anchor",
        ".src",
        "script",
        "style",
        ".sidebar",
        ".hideme",
    ]) {
        for (const n of main.querySelectorAll(sel)) {
            n.remove();
        }
    }
}

// Parse "slint/platform/enum.WindowEvent.html" -> path + kind + name.
export function classify(
    crate: string,
    rel: string,
): { path: string; kind: Kind; isIndex: boolean } | null {
    const parts = rel.split("/");
    const file = parts[parts.length - 1];
    const dirs = parts.slice(0, -1); // module segments below doc-root, incl. crate
    if (file === "index.html") {
        const mod = dirs.join("::");
        return { path: mod || crate, kind: "Module", isIndex: true };
    }
    const m = file.match(/^([a-z]+)\.(.+)\.html$/);
    if (!m) {
        return null;
    }
    const kind = FILE_KIND[m[1]];
    if (!kind) {
        return null;
    }
    return { path: [...dirs, m[2]].join("::"), kind, isIndex: false };
}

function processFile(
    docRoot: string,
    crate: string,
    file: string,
): Entry | null {
    const rel = relative(docRoot, file).split("\\").join("/");
    const info = classify(crate, rel);
    if (!info) {
        return null;
    }

    const root = parse(readFileSync(file, "utf8"));
    const main = root.querySelector("#main-content");
    if (!main) {
        return null;
    }
    prune(main);

    const docblock = main.querySelector(".docblock");
    const summary = docblock ? firstSentence(docblock.text) : "";

    let body: string;
    if (info.isIndex) {
        // Module / crate page: keep only the overview prose, not the item tables.
        body = docblock ? cleanup(render(docblock)) : "";
    } else {
        main.querySelector(".main-heading")?.remove();
        const decl = main.querySelector("pre.item-decl");
        let sig = "";
        if (decl) {
            sig = `\`\`\`rust\n${preText(decl).trim()}\n\`\`\`\n\n`;
            decl.remove();
        }
        body = sig + demoteHeadings(cleanup(render(main)));
    }

    const title = `## ${info.kind} \`${info.path}\``;
    const markdown = `${title}\n\n${body}`.trim();

    // Sort: crate root first, then by path; index pages before items in a module.
    const depth = info.path.split("::").length;
    const sortKey = `${info.isIndex ? depth : depth + 0.5}|${info.path}`;

    return {
        crate,
        path: info.path,
        kind: info.kind,
        url: rel,
        summary,
        markdown,
        sortKey,
    };
}

function walkHtml(dir: string, out: string[]): void {
    for (const name of readdirSync(dir, { withFileTypes: true })) {
        const full = join(dir, name.name);
        if (name.isDirectory()) {
            walkHtml(full, out);
        } else if (name.name.endsWith(".html") && name.name !== "all.html") {
            out.push(full);
        }
    }
}

function parseArgs(argv: string[]): Record<string, string> {
    const args: Record<string, string> = {};
    for (let i = 0; i < argv.length; i++) {
        if (argv[i].startsWith("--")) {
            args[argv[i].slice(2)] = argv[++i] ?? "";
        }
    }
    return args;
}

function buildIndex(entries: Entry[], crates: string[]): string {
    const lines: string[] = [
        "# Slint — Rust API",
        "",
        "> Curated index of the Slint Rust API reference (generated from rustdoc). " +
            "Rust is Slint's native binding. The full text is in llms-full.txt.",
        "",
    ];
    const KIND_ORDER: Kind[] = [
        "Module",
        "Macro",
        "Struct",
        "Enum",
        "Trait",
        "Function",
        "Type Alias",
        "Constant",
        "Static",
        "Union",
        "Primitive",
        "Derive Macro",
        "Attribute Macro",
        "Trait Alias",
    ];
    for (const crate of crates) {
        const items = entries.filter(
            (e) => e.crate === crate && e.kind !== "Module",
        );
        if (!items.length) {
            continue;
        }
        lines.push(`## ${crate}`, "");
        for (const kind of KIND_ORDER) {
            const group = items
                .filter((e) => e.kind === kind)
                .sort((a, b) => a.path.localeCompare(b.path));
            if (!group.length) {
                continue;
            }
            lines.push(`### ${kind}s`);
            for (const e of group) {
                lines.push(
                    `- [\`${e.path}\`](${e.url})${e.summary ? ` — ${e.summary}` : ""}`,
                );
            }
            lines.push("");
        }
    }
    lines.push(
        "## Also",
        "",
        "- [Full text](llms-full.txt)",
        "- [Slint language docs](../slint/llms.txt)",
        "- [docs.rs/slint](https://docs.rs/slint/)",
        "- [Slint website](https://slint.dev)",
        "- [Slint on GitHub](https://github.com/slint-ui/slint)",
        "",
    );
    return lines.join("\n");
}

function buildFull(entries: Entry[]): string {
    const head =
        "# Slint — Rust API (full text)\n\n" +
        "> Full text of the Slint Rust API reference, generated from rustdoc. " +
        "Covers item docs, signatures, inherent methods, variants and fields. " +
        "Auto/blanket trait implementations are omitted.\n";
    const body = entries
        .slice()
        .sort((a, b) =>
            a.sortKey.localeCompare(b.sortKey, "en", { numeric: true }),
        )
        .map((e) => e.markdown)
        .join("\n\n");
    return `${head}\n${body}\n`;
}

function main(): void {
    const args = parseArgs(process.argv.slice(2));
    const docRoot = args["doc-root"];
    if (!docRoot) {
        console.error("error: --doc-root <dir> is required");
        process.exit(2);
    }
    const out = args.out || docRoot;
    const crates = (args.crates || "slint,slint_interpreter,slint_build")
        .split(",")
        .map((c) => c.trim())
        .filter(Boolean);

    const entries: Entry[] = [];
    for (const crate of crates) {
        const dir = join(docRoot, crate);
        if (!existsSync(dir)) {
            console.warn(`warning: crate dir not found, skipping: ${dir}`);
            continue;
        }
        const files: string[] = [];
        walkHtml(dir, files);
        for (const f of files) {
            const e = processFile(docRoot, crate, f);
            if (e) {
                entries.push(e);
            }
        }
    }

    if (!entries.length) {
        console.error("error: no documented items found under " + docRoot);
        process.exit(1);
    }

    writeFileSync(join(out, "llms.txt"), buildIndex(entries, crates));
    writeFileSync(join(out, "llms-full.txt"), buildFull(entries));
    const items = entries.filter((e) => e.kind !== "Module").length;
    console.log(
        `rustdoc-to-llms: ${entries.length} pages (${items} items) from ${crates.join(", ")} -> ${out}/llms.txt, llms-full.txt`,
    );
}

if (
    process.argv[1] &&
    import.meta.url === pathToFileURL(process.argv[1]).href
) {
    main();
}
