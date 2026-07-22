// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Assign author-specified paragraph IDs to every `<p>` under `language/`.
// Each normative paragraph ends with a Pandoc-style marker, written as
// `\{#sls_xxx}` in the sources so that the brace also stays literal text in
// MDX files (where an unescaped `{` starts a JSX expression). The escape is
// consumed by the markdown parser, so this plugin sees `{#sls_xxx}`; it
// strips the marker, sets it as the paragraph's `id`, and appends a visible
// `[sls_xxx]` badge that doubles as an anchor.
//
// Cross-file references like `#sls_xxx` are validated by
// `starlight-links-validator`. A paragraph that loses its marker (e.g.
// after deletion) breaks any citation; collisions across the corpus
// fail the build.

const ID_MARKER = /\s*\{#(sls\.[a-z0-9.\-_]+)\}\s*$/;

function walk(node, fn) {
    if (node.type === "element") fn(node);
    if (node.children) {
        for (const child of node.children) walk(child, fn);
    }
}

export default function rehypeSlsIds() {
    // Closure-scoped: persists across files in one build, so a duplicate
    // id assigned in two different pages fails the build. The (id ->
    // sourcePath) mapping lets a dev-mode re-process of the same file
    // re-claim its own ids without false-positive collisions.
    const seen = new Map();

    return (tree, file) => {
        const sourcePath = file?.path ?? "";
        if (!sourcePath.includes("/language/")) return;

        // Tracks ids claimed during *this* invocation, so re-processing the
        // same file (dev-mode hot reload) re-claims its own ids cleanly while
        // intra-file duplicates still fail.
        const claimedHere = new Set();

        walk(tree, (node) => {
            if (node.tagName !== "p") return;
            const last = node.children.at(-1);
            if (!last || last.type !== "text") return;
            const match = last.value.match(ID_MARKER);
            if (!match) return;

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
            node.children.push({
                type: "element",
                tagName: "a",
                properties: { className: ["sls-id"], href: `#${id}` },
                children: [{ type: "text", value: `[${id}]` }],
            });
        });
    };
}
