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
// The pages that do carry identifiers are the specification and property-type
// chapters that opt into the safety corpus with `SC: true` and, in the safety
// manual, everything under `reference/`: the generated SC API reference and the
// chapters the manual writes itself, like rendering and generated code. Pass
// `{ referenceRequiresIds: true }` for the latter. They are also checked for
// completeness -- a normative paragraph without an identifier fails the build
// -- covering the paragraphs an `SC: true` page wraps in <SC>/<OnlyInSC>, the
// top-level paragraphs of the manual's own chapters (nested ones are asides and
// list items), and every paragraph of the generated reference. A page states no
// requirements, and so drops its markers instead of assigning them, in two
// cases: a specification chapter without `SC: true` documents the full language
// only, so the safety manual leaves it out, and a page with `normative: false`
// is navigational, like a section landing page. Dropping keeps the corpus and
// the traceability matrix in step: the matrix cites neither kind of page, and
// an anchor it never cites dead-links from nowhere and hides a marker that
// should have been checked. The markers stay in the source for when a chapter
// joins the subset.
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

// The specification and property-types chapters. A page in either joins the SC
// corpus by setting `SC: true`; this path also tells such a chapter apart from
// the generated and manual reference. Separator-agnostic so the checks also
// hold on Windows paths, and anchored on `content/docs/` so an unrelated
// directory in the checkout path can't match.
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
// any component does, so the paragraphs are still here to be skipped. It opts
// a part of a certified block back out, so it belongs inside one; a stray one
// fails the build, since outside a block the content is uncertified already
// and the tag would suggest the opposite.
const NOT_IN_SC = "NotInSC";
// Certified content is wrapped in <SC>. Inside it, <OnlyInSC> marks wording
// that holds for Slint SC alone (shown only in the safety manual); it is the
// counterpart of <NotInSC>, which opts a nested part back out (shown only in
// the main docs, e.g. a type description within a <SlintProperty>). Both <SC>
// and <OnlyInSC> bound a certified block.
const SC_TAGS = new Set(["SC", "OnlyInSC"]);

function walk(node, fn, depth = 0, inNotInSc = false, scDepth = null) {
    if (node.type === "element" || node.type === "mdxJsxFlowElement") {
        fn(node, depth, inNotInSc, scDepth);
    }
    let childNotInSc = inNotInSc;
    let childScDepth = scDepth;
    if (node.type === "mdxJsxFlowElement") {
        if (node.name === NOT_IN_SC) childNotInSc = true;
        // Track the nearest enclosing certified block, so a paragraph directly
        // inside an <OnlyInSC> nested in an <SC> is still checked.
        if (SC_TAGS.has(node.name)) childScDepth = depth;
    }
    for (const child of node.children ?? []) {
        walk(child, fn, depth + 1, childNotInSc, childScDepth);
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
        // A spec/property-types chapter opts into the safety corpus with
        // `SC: true`; only the content it then wraps in <SC> is certified.
        const isSC = isSpec && frontmatter?.SC === true;
        // A navigational page like a section landing page states no
        // requirements: it carries no identifiers and its markers are dropped.
        const statesNoRequirements = frontmatter?.normative === false;
        const assignsIds =
            (isSC || isGeneratedReference || isManualReference) &&
            !statesNoRequirements;
        // Draft pages aren't published, so they need no ids.
        const requireIds = assignsIds && !frontmatter?.draft;
        // The generated SC reference is normative at any depth. On an `SC: true`
        // page the normative paragraphs are the ones directly inside an
        // <SC>/<OnlyInSC> block; in the manual's own reference chapters, the
        // top-level paragraphs. Deeper paragraphs are asides and list items,
        // and a <NotInSC> nested inside a certified block is exempt.
        const requiredAtAnyDepth = isGeneratedReference;

        // Tracks ids claimed during *this* invocation, so re-processing the
        // same file (dev-mode hot reload) re-claims its own ids cleanly while
        // intra-file duplicates still fail.
        const claimedHere = new Set();
        const missing = [];
        const strayNotInSc = [];

        walk(tree, (node, depth, inNotInSc, scDepth) => {
            if (node.type === "mdxJsxFlowElement") {
                // <NotInSC> opts a part of a certified block back out, so it
                // belongs inside one. Outside, the content is uncertified
                // already and the tag reads as if the prose around it were
                // certified.
                if (isSC && node.name === NOT_IN_SC && scDepth === null) {
                    strayNotInSc.push(node.position?.start?.line);
                }
                return;
            }
            if (node.tagName !== "p") return;
            // Such a paragraph documents what Slint SC leaves out, so it
            // states no requirement and carries no identifier.
            if (inNotInSc) return;
            const last = node.children.at(-1);
            const match =
                last?.type === "text" ? last.value.match(ID_MARKER) : null;
            if (!match) {
                const normative =
                    requiredAtAnyDepth ||
                    (isManualReference && depth === 1) ||
                    (isSC && scDepth !== null && depth === scDepth + 1);
                if (requireIds && normative) {
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

        if (strayNotInSc.length > 0) {
            throw new Error(
                `rehype-sls-ids: <NotInSC> outside an <SC> block in ${sourcePath}\n` +
                    strayNotInSc.map((line) => `  line ${line}\n`).join("") +
                    "<NotInSC> shall be nested in the <SC> block whose content it opts out of.",
            );
        }

        if (missing.length > 0) {
            throw new Error(
                `rehype-sls-ids: ${missing.length} paragraph(s) without an id in ${sourcePath}\n` +
                    missing.map((p) => `  "${textPreview(p)}"\n`).join("") +
                    "Each normative paragraph shall end with a `\\{#sls.…}` marker.",
            );
        }
    };
}
