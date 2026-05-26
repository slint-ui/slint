// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore argsstring basecompoundref briefdescription codeline compounddef Doxyfile TAGFILES
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

/** A character range `[start, end)` in a signature that links to `url`. */
interface SignatureLink {
    start: number;
    end: number;
    url: string;
}

/** The bit of Shiki the converter uses; injected so the converter has no hard Shiki dependency. */
type SignatureHighlighter = Pick<import("shiki").Highlighter, "codeToHtml">;

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

// Base URL for cppreference.com symbols. Doxygen resolves standard-library
// references against the tag file configured in the Doxyfile (TAGFILES) and
// emits them as `<ref external="…" refid="cpp/…">`, where the refid is already
// the cppreference path. This base must match the URL given there.
const CPPREFERENCE_BASE = "https://en.cppreference.com/w/";

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
    /** Optional Shiki highlighter; when set, signatures are syntax-highlighted. */
    private readonly highlighter?: SignatureHighlighter;

    constructor(
        xmlDir: string,
        options: { highlighter?: SignatureHighlighter } = {},
    ) {
        this.xmlDir = xmlDir;
        this.highlighter = options.highlighter;
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

    /**
     * The header to advertise in a compound's `#include` line. Doxygen
     * attributes a class to the file its definition lives in, but Slint's
     * public sub-headers live under `private/` and are only meant to be pulled
     * in transitively via `slint.h`. Rewrite those to `slint.h`; leave the real
     * umbrella headers (slint.h, slint-platform.h, slint-interpreter.h,
     * slint-testing.h) as Doxygen reports them.
     */
    private displayInclude(inc: XmlElement): string {
        const name = textContent(inc).trim();
        const refid = inc.attrs.refid;
        if (refid) {
            const file = this.loadCompound(refid);
            const location = file && child(file, "location")?.attrs.file;
            if (location && /(^|\/)private\//.test(location)) {
                return "slint.h";
            }
        }
        return name;
    }

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

        const includes = [
            ...new Set(
                children(def, "includes")
                    .map((inc) => this.displayInclude(inc))
                    .filter(Boolean),
            ),
        ];
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
            (m) => m.attrs.prot !== "private" && !isInternalMember(m),
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

    /** Render a member signature: Shiki-highlighted with type links when a highlighter is set. */
    private renderSignature(member: XmlElement): string {
        const { text, links } = this.buildSignature(member);
        if (!this.highlighter) return signatureFallback(text, links);
        return this.highlighter.codeToHtml(text, {
            lang: "cpp",
            themes: { light: "light-plus", dark: "dark-plus" },
            defaultColor: false,
            decorations: links.map((l) => ({
                start: l.start,
                end: l.end,
                tagName: "a",
                properties: { href: l.url, class: "api-link" },
            })),
            transformers: [
                {
                    pre(node) {
                        const cls = node.properties.class;
                        node.properties.class = `${
                            typeof cls === "string" ? `${cls} ` : ""
                        }api-signature`;
                    },
                },
            ],
        });
    }

    /**
     * Reconstruct a member signature as plain text plus the character ranges of
     * cross-referenced types. The return type comes from `<type>` and the
     * parameter list from `<argsstring>`; the qualified name is a plain segment,
     * so a type that also appears in the name (a method of `Foo` taking a `Foo`)
     * is never mis-linked.
     */
    private buildSignature(member: XmlElement): {
        text: string;
        links: SignatureLink[];
    } {
        const kind = member.attrs.kind;
        const name = textContent(
            child(member, "name") ?? emptyElement(),
        ).trim();
        const links: SignatureLink[] = [];

        if (kind === "enum") {
            const scoped =
                member.attrs.strong === "yes" ? "enum class" : "enum";
            return { text: `${scoped} ${name}`, links };
        }

        const SPECIFIERS = [
            "virtual",
            "static",
            "explicit",
            "constexpr",
            "inline",
        ];
        let text = "";
        const prefix: string[] = [];
        if (member.attrs.explicit === "yes") prefix.push("explicit");
        if (member.attrs.static === "yes") prefix.push("static");
        if (member.attrs.constexpr === "yes") prefix.push("constexpr");
        if (
            member.attrs.virt === "virtual" ||
            member.attrs.virt === "pure-virtual"
        )
            prefix.push("virtual");
        if (prefix.length > 0) text += `${prefix.join(" ")} `;

        // Return type from <type>, recording links for resolvable refs.
        const typeStart = text.length;
        const typeEl = child(member, "type");
        if (typeEl) {
            for (const node of typeEl.children) {
                if (!isElement(node)) {
                    text += node.value;
                } else if (node.name === "ref") {
                    const t = textContent(node);
                    const url = this.resolveTargetUrl(
                        node.attrs.refid,
                        node.attrs.kindref,
                        node.attrs.external,
                    );
                    const start = text.length;
                    text += t;
                    if (url) links.push({ start, end: text.length, url });
                } else {
                    text += textContent(node);
                }
            }
        }
        if (text.length > typeStart) text += " ";

        // Qualified name = <definition> minus leading specifiers and return type.
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
        text += qualified || name;

        const args = textContent(child(member, "argsstring") ?? emptyElement());
        if (kind === "function" || kind === "friend") {
            const base = text.length;
            text += args;
            let pos = 0;
            for (const param of children(member, "param")) {
                const type = child(param, "type");
                if (!type) continue;
                for (const ref of collectRefs(type)) {
                    const t = textContent(ref).trim();
                    const url = this.resolveTargetUrl(
                        ref.attrs.refid,
                        ref.attrs.kindref,
                        ref.attrs.external,
                    );
                    if (!t || !url) continue;
                    const idx = args.indexOf(t, pos);
                    if (idx < 0) continue;
                    links.push({
                        start: base + idx,
                        end: base + idx + t.length,
                        url,
                    });
                    pos = idx + t.length;
                }
            }
        } else if (args) {
            text += args;
        }

        return { text, links };
    }

    /**
     * URL for a `<ref>` target: a relative link (with anchor) for a generated
     * page, or an absolute cppreference link for an external (standard-library)
     * reference. `external` is the `<ref>`'s `external` attribute, set by
     * Doxygen when the symbol was resolved through a tag file.
     */
    private resolveTargetUrl(
        refid?: string,
        kindref?: string,
        external?: string,
    ): string | undefined {
        if (!refid) return undefined;
        const target =
            this.memberTargets.get(refid) ??
            (kindref === "compound"
                ? this.compoundTargets.get(refid)
                : undefined) ??
            this.compoundTargets.get(refid);
        if (target) {
            const anchor = target.anchor ? `#${target.anchor}` : "";
            return `${relativeUrl(this.currentSlug, target.slug)}${anchor}`;
        }
        if (external) return `${CPPREFERENCE_BASE}${refid}`;
        return undefined;
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
        const url = this.resolveTargetUrl(
            refid,
            node.attrs.kindref,
            node.attrs.external,
        );
        if (!url) return `\`${text}\``;
        return `[${text}](${url})`;
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

/**
 * Render a signature as a plain `<pre>` block (no syntax highlighting), wrapping
 * the cross-referenced type ranges in `<a>` links. Used when no Shiki
 * highlighter is supplied (e.g. the unit tests).
 */
function signatureFallback(text: string, links: SignatureLink[]): string {
    const sorted = [...links].sort((a, b) => a.start - b.start);
    let html = "";
    let pos = 0;
    for (const link of sorted) {
        if (link.start < pos) continue;
        html += escapeHtml(text.slice(pos, link.start));
        html += `<a href="${link.url}">${escapeHtml(
            text.slice(link.start, link.end),
        )}</a>`;
        pos = link.end;
    }
    html += escapeHtml(text.slice(pos));
    return `<pre class="api-signature"><code>${html}</code></pre>`;
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
