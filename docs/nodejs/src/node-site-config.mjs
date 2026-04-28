// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const NODE_DOCS_BASE_URL = "https://localhost";

export const NODE_DOCS_BASE_PATH = "/";

/**
 * Absolute path from the origin for a file in `public/` (leading slash, no
 * trailing slash on the base segment). Used for `head` link `href`s because
 * those are not rewritten by Starlight the same way as sidebar `link`s.
 *
 * @param {string} path — e.g. `"favicon.svg"`
 */
export function nodeDocsPublicAsset(path) {
    const rel = path.startsWith("/") ? path.slice(1) : path;
    if (NODE_DOCS_BASE_PATH === "/") {
        return `/${rel}`;
    }
    const base = NODE_DOCS_BASE_PATH.replace(/\/+$/, "");
    return `${base}/${rel}`;
}
