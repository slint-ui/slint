// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import starlightLinksValidator from "starlight-links-validator";
import { rehypeExternalLinksSlint } from "./rehype-external-links-preset";

/** Trailing-slash policy shared across Slint Starlight doc sites. */
export const SLINT_STARLIGHT_TRAILING_SLASH = "always" as const;

/**
 * Default Astro `markdown.rehypePlugins` for Starlight sites (Slint external-link preset only).
 * Sites with extra rehype plugins should import {@link rehypeExternalLinksSlint} and compose locally.
 */
export function slintStarlightMarkdownRehypeExternalLinksOnly() {
    return {
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
