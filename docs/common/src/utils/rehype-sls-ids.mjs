// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Turn the author-specified paragraph identifiers of the normative pages into
// HTML ids. A normative paragraph ends with a Pandoc-style marker, written as
// `\{#sls.xxx}` in the sources so that the brace also stays literal text in
// MDX files (where an unescaped `{` starts a JSX expression). The escape is
// consumed by the markdown parser, so this plugin sees `{#sls.xxx}`; it strips
// the marker, sets it as the paragraph's `id`, and appends a visible
// `[sls.xxx]` badge that doubles as an anchor. The badge is what makes an
// identifier citable at a glance, which the safety manual needs and the main
// documentation doesn't: pass `{ renderBadge: false }` there to keep the
// identifiers as anchors without showing them.
//
// The markers are authoring syntax, so no page ever renders one verbatim:
// where a page carries no identifiers -- the same doc comments feed the main
// documentation, which has no traceability -- the marker is dropped instead.
// That keeps every producer of markers (the specification, builtins.slint doc
// comments, and whatever else grows one) free to write them unconditionally.
//
// The pages that do carry identifiers are the language specification and, in
// the safety manual, everything under `reference/`: the generated SC API
// reference and the chapters the manual writes itself, like rendering and
// generated code. Pass `{ referenceRequiresIds: true }` for those. They are
// also checked for completeness -- a normative paragraph without an
// identifier fails the build -- covering top-level paragraphs of the
// specification and of the manual's own chapters (nested ones are asides and
// list items) and every paragraph of the generated and property-types
// reference. A page states no requirements, and so drops its markers instead
// of assigning them, in two cases: a specification chapter with
// `notInSC: true` covers the full language only, so the safety manual leaves
// it out, and a page with `normative: false` is navigational, like a section
// landing page. Dropping keeps the corpus and the traceability matrix in
// step: the matrix cites neither kind of page, and an anchor it never cites
// dead-links from nowhere and hides a marker that should have been checked.
// The markers stay in the source for when a chapter joins the subset.
//
// The same marker format lives in `split_marker` in
// docs/slint-doc-generator/traceability.rs and in the `.sls-id` styling in
// docs/common/src/styles/sls-ids.css.
//
// Cross-file references like `#sls.xxx` are validated by
// `starlight-links-validator`. A paragraph that loses its marker (e.g.
// after deletion) breaks any citation; collisions across the corpus
// fail the build.

const ID_MARKER = /\s*\{#(sls\.[a-z0-9.\-_]+)\}\s*$/;

// The specification chapters and the property-types reference are the SC
// corpus: their paragraphs carry identifiers unless a page opts out with
// `notInSC: true`. Separator-agnostic so the checks also hold on Windows
// paths, and anchored on `content/docs/` so an unrelated directory in the
// checkout path can't match.
const SPEC_PATH =
    /[\\/]content[\\/]docs[\\/](reference[\\/])?(language|property-types)[\\/]/;
const GENERATED_REFERENCE_PATH =
    /[\\/]content[\\/]docs[\\/]generated[\\/]reference[\\/]/;
// The reference chapters the safety manual writes itself. The main
// documentation serves a much larger `reference/` that states no
// requirements, so this only applies where `referenceRequiresIds` is set. The
// manual's synced property-types pages sit below this path too, but they
// match SPEC_PATH first.
const MANUAL_REFERENCE_PATH = /[\\/]content[\\/]docs[\\/]reference[\\/]/;

// A feature outside the certified subset is wrapped in the `<NotInSC>`
// component: the safety manual renders nothing for it, but rehype runs before
// any component does, so the paragraphs are still here to be skipped.
const NOT_IN_SC = "NotInSC";

function walk(node, fn, depth = 0, inNotInSc = false) {
    if (node.type === "element") fn(node, depth, inNotInSc);
    const nested =
        inNotInSc ||
        (node.type === "mdxJsxFlowElement" && node.name === NOT_IN_SC);
    for (const child of node.children ?? []) {
        walk(child, fn, depth + 1, nested);
    }
}

/** Plain-text preview of a paragraph, for error messages. */
function textPreview(node) {
    let text = "";
    const collect = (n) => {
        if (n.type === "text") text += n.value;
        for (const child of n.children ?? []) collect(child);
    };
    collect(node);
    text = text.trim().replace(/\s+/g, " ");
    return text.length > 60 ? `${text.slice(0, 60)}…` : text;
}

export default function rehypeSlsIds({
    referenceRequiresIds = false,
    renderBadge = true,
} = {}) {
    // Closure-scoped: persists across files in one build, so a duplicate
    // id assigned in two different pages fails the build. The (id ->
    // sourcePath) mapping lets a dev-mode re-process of the same file
    // re-claim its own ids without false-positive collisions.
    const seen = new Map();

    return (tree, file) => {
        const sourcePath = file?.path ?? "";
        // The doc sites inline partials from each other (the main docs' enum
        // partials render inside the safety manual). Each site owns only the
        // pages below its own root, and decides the rules for those alone --
        // but a marker in a foreign partial still gets dropped rather than
        // rendered verbatim.
        const siteRoot = file?.cwd;
        const isOwnPage = !siteRoot || sourcePath.startsWith(siteRoot);
        const isSpec = isOwnPage && SPEC_PATH.test(sourcePath);
        // Both sites generate the reference from the same doc comments, but
        // only the safety manual treats it, and the reference chapters it
        // writes itself, as normative.
        const isReference = referenceRequiresIds && isOwnPage && !isSpec;
        const isGeneratedReference =
            isReference && GENERATED_REFERENCE_PATH.test(sourcePath);
        const isManualReference =
            isReference && MANUAL_REFERENCE_PATH.test(sourcePath);
        const frontmatter = file?.data?.astro?.frontmatter;
        // A chapter outside the SC subset, and a navigational page like a
        // section landing page, both state no requirements: they carry no
        // identifiers and their markers are dropped.
        const statesNoRequirements =
            (isSpec && Boolean(frontmatter?.notInSC)) ||
            frontmatter?.normative === false;
        const assignsIds =
            (isSpec || isGeneratedReference || isManualReference) &&
            !statesNoRequirements;
        // Draft pages aren't published, so they need no ids.
        const requireIds = assignsIds && !frontmatter?.draft;
        // In the language specification and the manual's own reference
        // chapters, only top-level paragraphs are normative: nested ones are
        // asides and list items. The generated SC reference and the
        // property-types reference are normative at any depth -- both wrap
        // normative prose in components like <SlintProperty>, so an untagged
        // nested paragraph there is a mistake and fails the build rather than
        // slipping through.
        const isPropertyTypes =
            isSpec && /[\\/]property-types[\\/]/.test(sourcePath);
        const requiredAtAnyDepth = isGeneratedReference || isPropertyTypes;

        // Tracks ids claimed during *this* invocation, so re-processing the
        // same file (dev-mode hot reload) re-claims its own ids cleanly while
        // intra-file duplicates still fail.
        const claimedHere = new Set();
        const missing = [];

        walk(tree, (node, depth, inNotInSc) => {
            if (node.tagName !== "p") return;
            // Such a paragraph documents what Slint SC leaves out, so it
            // states no requirement and carries no identifier.
            if (inNotInSc) return;
            const last = node.children.at(-1);
            const match =
                last?.type === "text" ? last.value.match(ID_MARKER) : null;
            if (!match) {
                if (requireIds && (requiredAtAnyDepth || depth === 1)) {
                    missing.push(node);
                }
                return;
            }

            // Drop the marker where this page carries no identifiers, so it
            // never reaches the reader as literal text.
            if (!assignsIds) {
                last.value = last.value.slice(0, -match[0].length);
                if (last.value === "") {
                    node.children.pop();
                }
                return;
            }

            const id = match[1];
            if (claimedHere.has(id)) {
                throw new Error(
                    `rehype-sls-ids: duplicate id ${id} within ${sourcePath}\n` +
                        "Each paragraph identifier shall be unique across the corpus.",
                );
            }
            claimedHere.add(id);

            const previousPath = seen.get(id);
            if (previousPath && previousPath !== sourcePath) {
                throw new Error(
                    `rehype-sls-ids: duplicate id ${id}\n` +
                        `  first defined in:  ${previousPath}\n` +
                        `  duplicated in:     ${sourcePath}\n` +
                        "Each paragraph identifier shall be unique across the corpus.",
                );
            }
            seen.set(id, sourcePath);

            last.value = last.value.slice(0, -match[0].length);
            if (last.value === "") {
                node.children.pop();
            }

            node.properties = { ...(node.properties || {}), id };
            // The identifier stays an anchor either way: the specification
            // cites paragraphs across chapters with `#sls.…` links.
            if (!renderBadge) return;
            node.children.push({
                type: "element",
                tagName: "a",
                properties: { className: ["sls-id"], href: `#${id}` },
                children: [{ type: "text", value: `[${id}]` }],
            });
        });

        if (missing.length > 0) {
            throw new Error(
                `rehype-sls-ids: ${missing.length} paragraph(s) without an id in ${sourcePath}\n` +
                    missing.map((p) => `  "${textPreview(p)}"\n`).join("") +
                    "Each normative paragraph shall end with a `\\{#sls.…}` marker.",
            );
        }
    };
}
