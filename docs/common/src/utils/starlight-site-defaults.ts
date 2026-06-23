// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import starlightLinksValidator from "starlight-links-validator";
import { rehypeExternalLinksSlint } from "./rehype-external-links-preset";

/** Trailing-slash policy shared across Slint Starlight doc sites. */
export const SLINT_STARLIGHT_TRAILING_SLASH = "always" as const;

/**
 * Default Astro `markdown` config for Starlight sites: the Slint external-link
 * rehype preset plus GitHub-Flavored Markdown.
 *
 * `gfm` is set explicitly because once an Astro site provides a `markdown`
 * object, the `@astrojs/mdx` pipeline no longer picks up the `gfm: true`
 * default, which silently disables GFM tables (and the rest of GFM) inside
 * `.mdx` pages. Keeping it here means every site that uses this helper renders
 * markdown tables instead of raw `|`-delimited text.
 *
 * Sites with extra rehype plugins should import {@link rehypeExternalLinksSlint}
 * and compose locally (and set `gfm: true` themselves).
 */
export function slintStarlightMarkdownRehypeExternalLinksOnly() {
    return {
        gfm: true,
        rehypePlugins: [rehypeExternalLinksSlint],
    };
}

/**
 * `starlight-links-validator` with Slint-wide defaults; pass Starlight-specific `exclude` / etc.
 */
export function slintStarlightLinksValidatorPlugin(
    options: Parameters<typeof starlightLinksValidator>[0] = {},
) {
    return starlightLinksValidator({
        errorOnLocalLinks: false,
        ...options,
    });
}
