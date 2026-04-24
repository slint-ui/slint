// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * Deploy path prefix for this site. Must stay in sync with Astro `base` when
 * PR4 wires production URLs (CI sed-patches this file like `docs/common`).
 * Use `"/"` for local dev (no `base` in `astro.config.mjs`).
 */
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
