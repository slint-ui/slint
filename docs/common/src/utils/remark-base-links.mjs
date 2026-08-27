// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Prefix the site base to the internal links of a page. Astro doesn't apply
// `base` to markdown links, so a site served under a path would need every
// link to spell that path out. Sources instead write links from the site root
// -- `/language/properties/` -- and this adds the base, in one place, at build
// time. That keeps the sources free of the deployment layout, which the
// release job rewrites per version.
//
// It matters for more than the output: starlight-links-validator resolves a
// link against the deployed path, so an unprefixed link fails validation as a
// link to an unknown page. Remark runs before the validator's rehype pass, so
// what gets checked is the prefixed link.

/** Rewrite the `url` of every link and link definition in the tree. */
function walk(node, fn) {
    if (node.type === "link" || node.type === "definition") {
        fn(node);
    }
    for (const child of node.children ?? []) {
        walk(child, fn);
    }
}

export default function remarkBaseLinks({ base = "/" } = {}) {
    const prefix = base.replace(/\/+$/, "");
    // Served at the root, so the links already read as they should.
    if (prefix === "") {
        return () => {};
    }
    return (tree) => {
        walk(tree, (node) => {
            // A protocol-relative URL also starts with a slash and leaves the
            // site, so it isn't ours to rewrite.
            if (node.url?.startsWith("/") && !node.url.startsWith("//")) {
                node.url = `${prefix}${node.url}`;
            }
        });
    };
}
