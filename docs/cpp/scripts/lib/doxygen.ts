// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore argsstring basecompoundref briefdescription codeline compounddef
// cSpell:ignore computeroutput declname derivedcompoundref detaileddescription enumvalue
// cSpell:ignore innerclass innernamespace itemizedlist kindref linkified linkify memberdef nonbreakablespace orderedlist
// cSpell:ignore parameterdescription parameteritem parameterlist parametername
// cSpell:ignore parameternamelist programlisting refid retval sectiondef simplesect virt
// cSpell:ignore templateparam templateparamlist tparams ulink xrefdescription xrefsect

// Converts a directory of Doxygen XML (the `xml/` output) into Markdown pages
// for the Astro/Starlight content collection. This is the C++ counterpart of
// what `starlight-typedoc` does for the Node.js API docs: it stands in for
// Breathe/Exhale by reading the same Doxygen XML and emitting docs the static
// site generator understands.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
    type XmlElement,
    type XmlNode,
    child,
    children,
    isElement,
    parseXml,
    textContent,
} from "./xml.ts";
import {
    compoundSlug,
    disambiguateAnchor,
    memberAnchorBase,
    relativeUrl,
} from "./slug.ts";

export interface IndexEntry {
    refid: string;
    kind: string;
    name: string;
}

export interface GeneratedPage {
    /** Slug relative to the content root, e.g. `api/classes/slint-color`. */
    slug: string;
    markdown: string;
}

/** Where a `refid` (compound or member) resolves to in the generated site. */
interface SymbolTarget {
    slug: string;
    anchor?: string;
}

/** Compound kinds we turn into their own page. */
const PAGE_KINDS = new Set([
    "class",
    "struct",
    "union",
    "interface",
    "namespace",
    "concept",
]);

const SECTION_TITLES: Record<string, string> = {
    "public-type": "Public Types",
    "public-func": "Public Functions",
    "public-static-func": "Public Static Functions",
    "public-attrib": "Public Attributes",
    "public-static-attrib": "Public Static Attributes",
    "protected-type": "Protected Types",
    "protected-func": "Protected Functions",
    "protected-attrib": "Protected Attributes",
    "private-func": "Private Functions",
    "private-attrib": "Private Attributes",
    friend: "Friends",
    related: "Related",
    "user-defined": "Members",
    typedef: "Typedefs",
    enum: "Enumerations",
    func: "Functions",
    var: "Variables",
    define: "Macros",
    "public-slot": "Public Slots",
};

const ASIDE_KIND: Record<string, string> = {
    note: "note",
    warning: "caution",
    attention: "caution",
    remark: "tip",
};

export class DoxygenConverter {
    private readonly xmlDir: string;
    /** Cache of parsed compound documents, keyed by refid. */
    private readonly compoundCache = new Map<string, XmlElement>();
    /** refid → page slug for every compound. */
    private readonly compoundTargets = new Map<string, SymbolTarget>();
    /** member id → { slug, anchor } so `<ref kindref="member">` resolves. */
    private readonly memberTargets = new Map<string, SymbolTarget>();
    /** Slug of the page currently being rendered, for relative cross-reference links. */
    private currentSlug = "";

    constructor(xmlDir: string) {
        this.xmlDir = xmlDir;
    }

    /** Parse `index.xml` into the list of compounds Doxygen produced. */
    readIndex(): IndexEntry[] {
        const index = parseXml(
            readFileSync(join(this.xmlDir, "index.xml"), "utf8"),
        );
        return children(index, "compound").map((c) => ({
            refid: c.attrs.refid,
            kind: c.attrs.kind,
            name: textContent(child(c, "name") ?? emptyElement()),
        }));
    }

    private loadCompound(refid: string): XmlElement | undefined {
        const cached = this.compoundCache.get(refid);
        if (cached) return cached;
        let source: string;
        try {
            source = readFileSync(join(this.xmlDir, `${refid}.xml`), "utf8");
        } catch {
            return undefined;
        }
        const doc = parseXml(source);
        const def = child(doc, "compounddef");
        if (!def) return undefined;
        this.compoundCache.set(refid, def);
        return def;
    }

    /** First pass: register every compound and member so cross-references resolve. */
    private buildSymbolMap(entries: IndexEntry[]): void {
        for (const entry of entries) {
            if (!PAGE_KINDS.has(entry.kind)) continue;
            const slug = compoundSlug(entry.kind, entry.name);
            this.compoundTargets.set(entry.refid, { slug });

            const def = this.loadCompound(entry.refid);
            if (!def) continue;
            const seen = new Map<string, number>();
            for (const section of children(def, "sectiondef")) {
                if (isHiddenSection(section.attrs.kind ?? "")) continue;
                for (const member of children(section, "memberdef")) {
                    if (isInternalMember(member)) continue;
                    const name = textContent(
                        child(member, "name") ?? emptyElement(),
                    );
                    const base = memberAnchorBase(name);
                    const occurrence = seen.get(base) ?? 0;
                    seen.set(base, occurrence + 1);
                    this.memberTargets.set(member.attrs.id, {
                        slug,
                        anchor: disambiguateAnchor(base, occurrence),
                    });
                }
            }
        }
    }

    /** Convert all compounds into pages. */
    convert(): GeneratedPage[] {
        const entries = this.readIndex();
        this.buildSymbolMap(entries);

        const pages: GeneratedPage[] = [];
        for (const entry of entries) {
            if (!PAGE_KINDS.has(entry.kind)) continue;
            const def = this.loadCompound(entry.refid);
            if (!def) continue;
            const target = this.compoundTargets.get(entry.refid);
            if (!target) continue;
            this.currentSlug = target.slug;
            pages.push({
                slug: target.slug,
                markdown: this.renderCompound(entry, def),
            });
        }
        return pages;
    }

    // --- page rendering -----------------------------------------------------

    private renderCompound(entry: IndexEntry, def: XmlElement): string {
        const brief = this.renderBlocks(child(def, "briefdescription"));
        const out: string[] = [];
        out.push("---");
        out.push(`title: ${frontmatterString(qualifiedTitle(entry))}`);
        const description = firstLine(brief);
        if (description)
            out.push(`description: ${frontmatterString(description)}`);
        out.push("---");
        out.push("");

        const tparams = this.renderTemplateLine(def);
        if (tparams) out.push("", "```cpp", tparams, "```", "");

        const includes = children(def, "includes")
            .map((inc) => textContent(inc).trim())
            .filter(Boolean);
        if (includes.length > 0) {
            out.push(
                "",
                "```cpp",
                ...includes.map((inc) => `#include <${inc}>`),
                "```",
                "",
            );
        }

        out.push(...this.renderInheritance(def));

        if (brief.trim()) out.push("", brief);
        const detailed = this.renderBlocks(child(def, "detaileddescription"));
        if (detailed.trim()) out.push("", detailed);

        out.push(...this.renderInnerCompounds(entry, def));

        for (const section of children(def, "sectiondef")) {
            if (isHiddenSection(section.attrs.kind ?? "")) continue;
            out.push(...this.renderSection(section));
        }

        return `${out
            .join("\n")
            .replace(/\n{3,}/g, "\n\n")
            .trimEnd()}\n`;
    }

    private renderTemplateLine(def: XmlElement): string | undefined {
        const list = child(def, "templateparamlist");
        if (!list) return undefined;
        const params = children(list, "param").map((p) => {
            const type = textContent(child(p, "type") ?? emptyElement()).trim();
            const declname = textContent(
                child(p, "declname") ?? emptyElement(),
            ).trim();
            return declname ? `${type} ${declname}` : type;
        });
        return `template <${params.join(", ")}>`;
    }

    private renderInheritance(def: XmlElement): string[] {
        const lines: string[] = [];
        const bases = children(def, "basecompoundref")
            .map((b) => this.linkForCompoundRef(b))
            .filter(Boolean);
        const derived = children(def, "derivedcompoundref")
            .map((b) => this.linkForCompoundRef(b))
            .filter(Boolean);
        if (bases.length > 0)
            lines.push("", `**Inherits** ${bases.join(", ")}.`);
        if (derived.length > 0)
            lines.push("", `**Inherited by** ${derived.join(", ")}.`);
        return lines;
    }

    private linkForCompoundRef(ref: XmlElement): string {
        const text = textContent(ref).trim();
        const refid = ref.attrs.refid;
        const target = refid ? this.compoundTargets.get(refid) : undefined;
        return target
            ? `[${text}](${relativeUrl(this.currentSlug, target.slug)})`
            : `\`${text}\``;
    }

    /** List the nested namespaces and classes/structs of a compound, with links to their pages. */
    private renderInnerCompounds(entry: IndexEntry, def: XmlElement): string[] {
        const out: string[] = [];
        const link = (el: XmlElement): string => {
            const name = textContent(el).trim();
            const display = name.startsWith(`${entry.name}::`)
                ? name.slice(entry.name.length + 2)
                : name;
            const target = this.compoundTargets.get(el.attrs.refid);
            return target
                ? `[${display}](${relativeUrl(this.currentSlug, target.slug)})`
                : `\`${display}\``;
        };

        const namespaces = children(def, "innernamespace");
        if (namespaces.length > 0) {
            out.push("", "## Namespaces");
            for (const n of namespaces) out.push(`- ${link(n)}`);
        }
        const classes = children(def, "innerclass").filter(
            (c) => c.attrs.prot !== "private",
        );
        if (classes.length > 0) {
            out.push("", "## Classes");
            for (const c of classes) out.push(`- ${link(c)}`);
        }
        return out;
    }

    private renderSection(section: XmlElement): string[] {
        const members = children(section, "memberdef").filter(
            (m) => !isInternalMember(m),
        );
        if (members.length === 0) return [];
        const kind = section.attrs.kind ?? "";
        const headerEl = child(section, "header");
        const title = headerEl
            ? textContent(headerEl).trim()
            : (SECTION_TITLES[kind] ?? "Members");
        const out: string[] = ["", `## ${title}`];
        for (const member of members) out.push(...this.renderMember(member));
        return out;
    }

    private renderMember(member: XmlElement): string[] {
        const name = textContent(
            child(member, "name") ?? emptyElement(),
        ).trim();
        const target = this.memberTargets.get(member.attrs.id);
        const anchor = target?.anchor ?? name;
        const out: string[] = [
            "",
            `### <a id="${anchor}"></a> \`${name}\``,
            "",
        ];

        out.push("", this.renderSignature(member));

        const enumValues = children(member, "enumvalue");
        if (enumValues.length > 0) {
            out.push("", "| Value | Description |", "| --- | --- |");
            for (const ev of enumValues) {
                const evName = textContent(
                    child(ev, "name") ?? emptyElement(),
                ).trim();
                const evDoc =
                    firstLine(
                        this.renderBlocks(child(ev, "briefdescription")),
                    ) || "";
                out.push(`| \`${evName}\` | ${escapeTableCell(evDoc)} |`);
            }
        }

        const brief = this.renderBlocks(child(member, "briefdescription"));
        if (brief.trim()) out.push("", brief);
        const detailed = this.renderBlocks(
            child(member, "detaileddescription"),
        );
        if (detailed.trim()) out.push("", detailed);
        return out;
    }

    /**
     * Render a member's signature as an HTML `<pre><code>` block where type
     * references resolve to links. The return type comes from `<type>` and the
     * parameter list from `<argsstring>` (both linkified); the qualified name is
     * a plain non-linkified segment, so a type that also appears in the name
     * (e.g. a method of `Foo` taking a `Foo`) is never mis-linked. A raw `<pre>`
     * HTML block is used so Markdown does not reinterpret signature punctuation.
     */
    private renderSignature(member: XmlElement): string {
        const kind = member.attrs.kind;
        const name = textContent(
            child(member, "name") ?? emptyElement(),
        ).trim();

        if (kind === "enum") {
            const scoped =
                member.attrs.strong === "yes" ? "enum class" : "enum";
            return signatureBlock(escapeHtml(`${scoped} ${name}`));
        }

        const SPECIFIERS = [
            "virtual",
            "static",
            "explicit",
            "constexpr",
            "inline",
        ];
        const prefix: string[] = [];
        if (member.attrs.explicit === "yes") prefix.push("explicit");
        if (member.attrs.static === "yes") prefix.push("static");
        if (member.attrs.constexpr === "yes") prefix.push("constexpr");
        if (
            member.attrs.virt === "virtual" ||
            member.attrs.virt === "pure-virtual"
        )
            prefix.push("virtual");

        const returnType = this.renderTypeHtml(child(member, "type"));

        // Qualified name = <definition> with the leading specifiers and return
        // type stripped off (so template parameters in the name are kept).
        let qualified = textContent(
            child(member, "definition") ?? emptyElement(),
        ).trim();
        let changed = true;
        while (changed) {
            changed = false;
            for (const sp of SPECIFIERS) {
                if (qualified.startsWith(`${sp} `)) {
                    qualified = qualified.slice(sp.length + 1);
                    changed = true;
                }
            }
        }
        const returnTypeText = textContent(
            child(member, "type") ?? emptyElement(),
        ).trim();
        if (returnTypeText && qualified.startsWith(`${returnTypeText} `)) {
            qualified = qualified.slice(returnTypeText.length + 1);
        }
        const nameSegment = escapeHtml(qualified || name);

        let html = "";
        if (prefix.length > 0) html += `${escapeHtml(prefix.join(" "))} `;
        if (returnType) html += `${returnType} `;
        html += nameSegment;

        if (kind === "function" || kind === "friend") {
            html += this.linkifyArgsString(member);
        } else {
            const args = textContent(
                child(member, "argsstring") ?? emptyElement(),
            ).trim();
            if (args) html += escapeHtml(args);
        }
        return signatureBlock(html);
    }

    /** Render a `<type>` element as escaped HTML, turning resolvable `<ref>`s into links. */
    private renderTypeHtml(el: XmlElement | undefined): string {
        if (!el) return "";
        const render = (node: XmlNode): string => {
            if (!isElement(node)) return escapeHtml(node.value);
            if (node.name === "ref") {
                const url = this.resolveTargetUrl(
                    node.attrs.refid,
                    node.attrs.kindref,
                );
                const text = escapeHtml(textContent(node));
                return url ? `<a href="${url}">${text}</a>` : text;
            }
            return node.children.map(render).join("");
        };
        return el.children.map(render).join("").trim();
    }

    /** The `<argsstring>` ("(params) const …") with parameter type refs linkified, escaped. */
    private linkifyArgsString(member: XmlElement): string {
        const args = textContent(child(member, "argsstring") ?? emptyElement());
        const refs: { text: string; url: string }[] = [];
        for (const param of children(member, "param")) {
            const type = child(param, "type");
            if (!type) continue;
            for (const ref of collectRefs(type)) {
                const text = textContent(ref).trim();
                const url = this.resolveTargetUrl(
                    ref.attrs.refid,
                    ref.attrs.kindref,
                );
                if (text && url) refs.push({ text, url });
            }
        }
        let html = "";
        let pos = 0;
        for (const ref of refs) {
            const idx = args.indexOf(ref.text, pos);
            if (idx < 0) continue;
            html += escapeHtml(args.slice(pos, idx));
            html += `<a href="${ref.url}">${escapeHtml(ref.text)}</a>`;
            pos = idx + ref.text.length;
        }
        html += escapeHtml(args.slice(pos));
        return html;
    }

    /** Relative URL (with anchor) for a `<ref>` target that resolves to a generated page. */
    private resolveTargetUrl(
        refid?: string,
        kindref?: string,
    ): string | undefined {
        if (!refid) return undefined;
        const target =
            this.memberTargets.get(refid) ??
            (kindref === "compound"
                ? this.compoundTargets.get(refid)
                : undefined) ??
            this.compoundTargets.get(refid);
        if (!target) return undefined;
        const anchor = target.anchor ? `#${target.anchor}` : "";
        return `${relativeUrl(this.currentSlug, target.slug)}${anchor}`;
    }

    // --- description rendering ---------------------------------------------

    /** Render an optional description element as block-level Markdown. */
    private renderBlocks(element: XmlElement | undefined): string {
        if (!element) return "";
        return this.renderBlockChildren(element.children);
    }

    private renderBlockChildren(nodes: XmlNode[]): string {
        const blocks: string[] = [];
        for (const node of nodes) {
            const rendered = this.renderBlockNode(node);
            if (rendered.trim()) blocks.push(rendered.trim());
        }
        return blocks.join("\n\n");
    }

    /** A `\section`/Markdown-heading block (`<sect1>`…`<sect4>`): a title plus block content. */
    private renderDocSection(node: XmlElement): string {
        const levels: Record<string, number> = {
            sect1: 3,
            sect2: 4,
            sect3: 5,
            sect4: 6,
        };
        const level = levels[node.name] ?? 4;
        const parts: string[] = [];
        const titleEl = child(node, "title");
        if (titleEl) {
            const title = this.inline(titleEl.children).trim();
            if (title) parts.push(`${"#".repeat(level)} ${title}`);
        }
        const body = node.children.filter(
            (c) => !(isElement(c) && c.name === "title"),
        );
        const rendered = this.renderBlockChildren(body);
        if (rendered) parts.push(rendered);
        return parts.join("\n\n");
    }

    private renderBlockNode(node: XmlNode): string {
        if (!isElement(node))
            return node.value.trim() ? this.inline([node]) : "";
        switch (node.name) {
            case "para":
                return this.renderParaWithBlocks(node);
            case "sect1":
            case "sect2":
            case "sect3":
            case "sect4":
                return this.renderDocSection(node);
            case "itemizedlist":
                return this.renderList(node, false);
            case "orderedlist":
                return this.renderList(node, true);
            case "programlisting":
                return this.renderProgramListing(node);
            case "verbatim":
                return [
                    "```",
                    textContent(node).replace(/\n$/, ""),
                    "```",
                ].join("\n");
            case "simplesect":
                return this.renderSimpleSect(node);
            case "parameterlist":
                return this.renderParameterList(node);
            case "heading": {
                const level = Number.parseInt(node.attrs.level ?? "3", 10);
                return `${"#".repeat(Math.min(Math.max(level, 1), 6))} ${this.inline(node.children)}`;
            }
            case "xrefsect":
                return this.renderBlocks(child(node, "xrefdescription"));
            default:
                return this.inline(node.children);
        }
    }

    /**
     * A `<para>` may contain inline runs interleaved with block elements
     * (lists, code, admonitions). Split so blocks render on their own.
     */
    private renderParaWithBlocks(para: XmlElement): string {
        const BLOCK_NAMES = new Set([
            "itemizedlist",
            "orderedlist",
            "programlisting",
            "verbatim",
            "simplesect",
            "parameterlist",
            "heading",
            "xrefsect",
        ]);
        const segments: string[] = [];
        let inlineRun: XmlNode[] = [];
        const flush = (): void => {
            if (inlineRun.length === 0) return;
            const text = this.inline(inlineRun).trim();
            if (text) segments.push(text);
            inlineRun = [];
        };
        for (const node of para.children) {
            if (isElement(node) && BLOCK_NAMES.has(node.name)) {
                flush();
                const block = this.renderBlockNode(node);
                if (block.trim()) segments.push(block.trim());
            } else {
                inlineRun.push(node);
            }
        }
        flush();
        return segments.join("\n\n");
    }

    private renderList(list: XmlElement, ordered: boolean): string {
        const items = children(list, "listitem").map((item, index) => {
            const body = children(item, "para")
                .map((p) => this.inline(p.children).trim())
                .filter(Boolean)
                .join(" ");
            const marker = ordered ? `${index + 1}.` : "-";
            return `${marker} ${body}`;
        });
        return items.join("\n");
    }

    private renderProgramListing(node: XmlElement): string {
        const lines = children(node, "codeline").map((line) => codeText(line));
        return ["```cpp", lines.join("\n").replace(/\n$/, ""), "```"].join(
            "\n",
        );
    }

    private renderSimpleSect(node: XmlElement): string {
        const kind = node.attrs.kind ?? "";
        const body = this.renderBlocks(node).trim();
        if (kind === "return") return `**Returns:** ${collapse(body)}`;
        if (kind === "see") return `**See also:** ${collapse(body)}`;
        if (kind === "since") return `**Since:** ${collapse(body)}`;
        if (kind === "par") {
            const titleEl = child(node, "title");
            const title = titleEl ? textContent(titleEl).trim() : "";
            return title ? `**${title}**\n\n${body}` : body;
        }
        const aside = ASIDE_KIND[kind];
        if (aside) return `:::${aside}\n${body}\n:::`;
        return body;
    }

    private renderParameterList(node: XmlElement): string {
        const kindLabel: Record<string, string> = {
            param: "Parameters",
            retval: "Return values",
            exception: "Exceptions",
            templateparam: "Template parameters",
        };
        const label = kindLabel[node.attrs.kind ?? "param"] ?? "Parameters";
        const rows: string[] = [];
        for (const item of children(node, "parameteritem")) {
            const names = children(item, "parameternamelist")
                .flatMap((nl) => children(nl, "parametername"))
                .map((pn) => textContent(pn).trim())
                .filter(Boolean);
            const desc = collapse(
                this.renderBlocks(child(item, "parameterdescription")).trim(),
            );
            const namePart = names.map((n) => `\`${n}\``).join(", ");
            rows.push(`- ${namePart}${desc ? ` — ${desc}` : ""}`);
        }
        if (rows.length === 0) return "";
        return `**${label}:**\n\n${rows.join("\n")}`;
    }

    // --- inline rendering ---------------------------------------------------

    private inline(nodes: XmlNode[]): string {
        return nodes.map((node) => this.inlineNode(node)).join("");
    }

    private inlineNode(node: XmlNode): string {
        if (!isElement(node))
            return escapeInline(node.value.replace(/\s+/g, " "));
        switch (node.name) {
            case "ref":
                return this.renderRef(node);
            case "computeroutput":
                return `\`${stripTicks(textContent(node))}\``;
            case "bold":
            case "b":
                return `**${this.inline(node.children).trim()}**`;
            case "emphasis":
            case "em":
                return `*${this.inline(node.children).trim()}*`;
            case "ulink":
                return `[${this.inline(node.children).trim()}](${node.attrs.url ?? ""})`;
            case "linebreak":
                return "  \n";
            case "sp":
                return " ";
            case "nonbreakablespace":
                return " ";
            case "anchor":
                return "";
            case "para":
                return this.inline(node.children);
            default:
                return this.inline(node.children);
        }
    }

    private renderRef(node: XmlElement): string {
        const text =
            this.inline(node.children).trim() || textContent(node).trim();
        const refid = node.attrs.refid;
        if (!refid) return text;
        const target =
            this.memberTargets.get(refid) ??
            (node.attrs.kindref === "compound"
                ? this.compoundTargets.get(refid)
                : undefined) ??
            this.compoundTargets.get(refid);
        if (!target) return `\`${text}\``;
        const anchor = target.anchor ? `#${target.anchor}` : "";
        return `[${text}](${relativeUrl(this.currentSlug, target.slug)}${anchor})`;
    }
}

// --- helpers ----------------------------------------------------------------

function emptyElement(): XmlElement {
    return { type: "element", name: "#empty", attrs: {}, children: [] };
}

/** Escape text for inclusion in raw HTML. */
function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

/** All `<ref>` elements within a node, recursively. */
function collectRefs(node: XmlElement): XmlElement[] {
    const refs: XmlElement[] = [];
    const walk = (n: XmlNode): void => {
        if (!isElement(n)) return;
        if (n.name === "ref") refs.push(n);
        for (const c of n.children) walk(c);
    };
    for (const c of node.children) walk(c);
    return refs;
}

/** Wrap rendered signature HTML in a raw `<pre>` block (passed through by Markdown). */
function signatureBlock(inner: string): string {
    return `<pre class="api-signature"><code>${inner}</code></pre>`;
}

/** Private members are implementation detail and excluded from the public API docs. */
function isHiddenSection(kind: string): boolean {
    return kind.startsWith("private");
}

/**
 * A member whose own qualified name lives in an internal namespace
 * (`private_api`/`cbindgen_private`). Doxygen's EXCLUDE_SYMBOLS drops the
 * standalone pages for those, but it can't strip e.g. a `friend` declaration to
 * such a function inside a public class — so filter them here too.
 */
function isInternalMember(member: XmlElement): boolean {
    const name = textContent(child(member, "name") ?? emptyElement());
    return (
        name.includes("private_api::") || name.includes("cbindgen_private::")
    );
}

function qualifiedTitle(entry: IndexEntry): string {
    const kindLabel: Record<string, string> = {
        class: "Class",
        struct: "Struct",
        union: "Union",
        interface: "Interface",
        namespace: "Namespace",
        concept: "Concept",
    };
    const label = kindLabel[entry.kind];
    return label ? `${entry.name} ${label}` : entry.name;
}

function frontmatterString(value: string): string {
    return JSON.stringify(value);
}

function firstLine(markdown: string): string {
    const line = markdown.split("\n").find((l) => l.trim().length > 0) ?? "";
    return collapse(line.trim());
}

function collapse(text: string): string {
    return text.replace(/\s+/g, " ").trim();
}

function escapeInline(text: string): string {
    // Escape Markdown control characters that would otherwise be interpreted.
    return text.replace(/([\\`*_{}\[\]<>])/g, "\\$1");
}

function escapeTableCell(text: string): string {
    return text.replace(/\|/g, "\\|");
}

function stripTicks(text: string): string {
    return text.replace(/`/g, "");
}

/** Verbatim text of a `<codeline>`, turning Doxygen's `<sp/>` markers into spaces. */
function codeText(node: XmlNode): string {
    if (!isElement(node)) return node.value;
    if (node.name === "sp") {
        const count = Number.parseInt(node.attrs.value ?? "1", 10);
        return " ".repeat(Number.isNaN(count) ? 1 : Math.max(count, 1));
    }
    return node.children.map(codeText).join("");
}
