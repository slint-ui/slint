// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Auto-assign stable, content-derived paragraph IDs to every `<p>` under
// `language/`. The ID is a 6-character truncation of the SHA-256 of the
// paragraph's normalized text, prefixed with `sls_`. Cross-file references
// like `#sls_3a7c9f` are validated by `starlight-links-validator`, so any
// substantive edit to a cited paragraph fails the build at the citing site.

import { createHash } from "node:crypto";

function collectText(node, out) {
    if (node.type === "text") {
        out.push(node.value);
        return;
    }
    if (node.children) {
        for (const child of node.children) collectText(child, out);
    }
}

function paragraphText(node) {
    const out = [];
    collectText(node, out);
    return out.join("").replace(/\s+/g, " ").trim();
}

function walk(node, fn) {
    if (node.type === "element") fn(node);
    if (node.children) {
        for (const child of node.children) walk(child, fn);
    }
}

export default function rehypeSlsIds() {
    return (tree, file) => {
        const sourcePath = file?.path ?? "";
        if (!sourcePath.includes("/language/")) return;

        const seen = new Map();
        walk(tree, (node) => {
            if (node.tagName !== "p") return;
            const text = paragraphText(node);
            if (!text) return;

            const hash = createHash("sha256").update(text).digest("hex").slice(0, 6);
            const id = `sls_${hash}`;

            const previous = seen.get(id);
            if (previous && previous !== text) {
                throw new Error(
                    `rehype-sls-ids: hash collision for ${id} in ${sourcePath}\n` +
                        `  existing: ${JSON.stringify(previous)}\n` +
                        `  new:      ${JSON.stringify(text)}\n` +
                        `Reword one of the paragraphs to break the collision.`,
                );
            }
            seen.set(id, text);

            node.properties = node.properties || {};
            node.properties.id = id;
            node.children.push({
                type: "element",
                tagName: "a",
                properties: { className: ["sls-id"], href: `#${id}` },
                children: [{ type: "text", value: `[${id}]` }],
            });
        });
    };
}
