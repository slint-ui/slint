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
// the safety manual, the generated SC API reference: pass
// `{ generatedReferenceRequiresIds: true }` for the latter. Those pages are
// also checked for completeness -- a normative paragraph without an
// identifier fails the build -- covering top-level paragraphs of the
// specification (nested ones are asides and list items) and every paragraph
// of the generated reference.
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

// Separator-agnostic so the checks also hold on Windows paths, and anchored on
// `content/docs/` so an unrelated directory in the checkout path can't match.
const SPEC_PATH = /[\\/]content[\\/]docs[\\/](reference[\\/])?language[\\/]/;
const GENERATED_REFERENCE_PATH =
    /[\\/]content[\\/]docs[\\/]generated[\\/]reference[\\/]/;

function walk(node, fn, depth = 0) {
    if (node.type === "element") fn(node, depth);
    for (const child of node.children ?? []) {
        walk(child, fn, depth + 1);
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
    generatedReferenceRequiresIds = false,
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
        // only the safety manual treats it as normative.
        const isNormativeReference =
            generatedReferenceRequiresIds &&
            isOwnPage &&
            !isSpec &&
            GENERATED_REFERENCE_PATH.test(sourcePath);
        const assignsIds = isSpec || isNormativeReference;

        // Draft pages aren't published, so they need no ids.
        const requireIds = assignsIds && !file?.data?.astro?.frontmatter?.draft;
        // In the specification, only top-level paragraphs are normative:
        // nested ones are asides and list items. Every paragraph of the
        // generated SC reference is normative, at any depth.
        const requiredAtAnyDepth = isNormativeReference;

        // Tracks ids claimed during *this* invocation, so re-processing the
        // same file (dev-mode hot reload) re-claims its own ids cleanly while
        // intra-file duplicates still fail.
        const claimedHere = new Set();
        const missing = [];

        walk(tree, (node, depth) => {
            if (node.tagName !== "p") return;
            // A feature outside the certified subset states no requirement, so
            // it carries no identifier. See rehype-not-in-sc.mjs.
            if (node.data?.notInSc) return;
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
