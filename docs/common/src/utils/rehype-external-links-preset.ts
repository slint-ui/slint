// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import rehypeExternalLinks from "rehype-external-links";
import type { Options } from "rehype-external-links";

const options: Options = {
    content: {
        type: "text",
        value: " ↗",
    },
    properties: {
        target: "_blank",
    },
    rel: ["noopener"],
};

/** Rehype plugin entry for Starlight / Astro markdown: external links open in a new tab with an indicator. */
export const rehypeExternalLinksSlint: [typeof rehypeExternalLinks, Options] = [
    rehypeExternalLinks,
    options,
];
