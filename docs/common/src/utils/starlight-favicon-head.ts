// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Default Safari pinned-tab mask color for Slint favicon SVG. */
const SLINT_STARLIGHT_FAVICON_MASK_COLOR = "#8D46E7";

/**
 * Starlight `head` link entries for the standard Slint documentation favicon set.
 *
 * @param hrefForPublicFile - Maps a `public/` filename to a URL path (leading slash; include
 *   site `base` when deployed under a subpath).
 */
export function slintStarlightFaviconHead(
    hrefForPublicFile: (filename: string) => string,
    options: { maskColor?: string } = {},
) {
    const maskColor = options.maskColor ?? SLINT_STARLIGHT_FAVICON_MASK_COLOR;
    const h = hrefForPublicFile;
    return [
        {
            tag: "link" as const,
            attrs: {
                rel: "icon",
                type: "image/svg+xml",
                href: h("favicon.svg"),
            },
        },
        {
            tag: "link" as const,
            attrs: {
                rel: "icon",
                type: "image/png",
                sizes: "32x32",
                href: h("favicon-32x32.png"),
            },
        },
        {
            tag: "link" as const,
            attrs: {
                rel: "icon",
                type: "image/png",
                sizes: "16x16",
                href: h("favicon-16x16.png"),
            },
        },
        {
            tag: "link" as const,
            attrs: {
                rel: "icon",
                type: "image/x-icon",
                href: h("favicon.ico"),
            },
        },
        {
            tag: "link" as const,
            attrs: {
                rel: "mask-icon",
                href: h("favicon.svg"),
                color: maskColor,
            },
        },
        {
            tag: "link" as const,
            attrs: {
                rel: "apple-touch-icon",
                sizes: "180x180",
                href: h("apple-touch-icon.png"),
            },
        },
    ];
}
