// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import linkData from "../../../../internal/core-macros/link-data.json" with {
    type: "json",
};

const links: Readonly<Record<string, { href: string }>> = linkData;

// Doxygen leaves a Markdown link with an unknown scheme as plain text,
// which the converter escapes: `\[label\](slint:key)`, with `_` escaped too.
const ESCAPED_SLINT_LINK = /\\\[(.+?)\\\]\(slint:((?:[A-Za-z0-9-]|\\_)+)\)/g;

/**
 * Resolves `[label](slint:key)` links in a doc comment to the Slint docs page that
 * `internal/core-macros/link-data.json` maps `key` to, like `#[slint_doc]` does for Rust.
 */
export function resolveSlintLinks(
    markdown: string,
    slintDocsBase: string,
    page: string,
): string {
    return markdown.replace(ESCAPED_SLINT_LINK, (_match, label, escapedKey) => {
        const key = escapedKey.replace(/\\_/g, "_");
        const link = links[key];
        if (!link) {
            throw new Error(
                `${page}: unknown link "slint:${key}". Add it to internal/core-macros/link-data.json.`,
            );
        }
        return `[${label}](${slintDocsBase}${link.href})`;
    });
}
