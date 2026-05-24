// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const PYTHON_DOCS_BASE_URL = "https://localhost";

export const PYTHON_DOCS_BASE_PATH = "/";

/**
 * Absolute path from the origin for a file in `public/` (leading slash, no
 * trailing slash on the base segment). Used for `head` link `href`s because
 * those are not rewritten by Starlight the same way as sidebar `link`s.
 *
 * @param {string} path — e.g. `"favicon.svg"`
 */
export function pythonDocsPublicAsset(path) {
    const rel = path.startsWith("/") ? path.slice(1) : path;
    if (PYTHON_DOCS_BASE_PATH === "/") {
        return `/${rel}`;
    }
    const base = PYTHON_DOCS_BASE_PATH.replace(/\/+$/, "");
    return `${base}/${rel}`;
}

/**
 * Absolute base URL of the Slint language docs, a sibling of this site under
 * the same origin and version (…/docs/slint/ next to …/docs/python/). Passed
 * to the shared Link component by the SlintRef wrapper for cross-references.
 */
export function slintDocsBase() {
    const origin = String(PYTHON_DOCS_BASE_URL).replace(/\/+$/, "");
    const slintBase = PYTHON_DOCS_BASE_PATH.replace(/python\/?$/, "slint/");
    return `${origin}${slintBase}`;
}
