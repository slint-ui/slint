// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import { slintStarlightFaviconHead } from "@slint/common-files/src/utils/starlight-favicon-head";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { rehypeExternalLinksSlint } from "@slint/common-files/src/utils/rehype-external-links-preset";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";

import { unified } from "@astrojs/markdown-remark";

const base = process.env.MATERIAL_DOCS_BASE_PATH || "/";

// https://astro.build/config
export default defineConfig({
    site: "https://material.slint.dev",
    base,
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: {
        processor: unified({ rehypePlugins: [rehypeExternalLinksSlint] }),
    },
    integrations: [
        starlight({
            title: "Slint Material Components",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: [
                "@slint/common-files/src/styles/starlight-slint-custom.css",
                "@slint/common-files/src/styles/starlight-slint-theme.css",
            ],
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            sidebar: [
                { label: "Getting Started", link: "getting-started" },
                {
                    label: "Components",
                    items: [{ autogenerate: { directory: "components" } }],
                },
            ],
            plugins: [
                slintStarlightLinksValidatorPlugin({
                    exclude: ["/zip/**"],
                }),
            ],
            social: slintStarlightSocial,
            favicon: "favicon.svg",
            head: slintStarlightFaviconHead((filename) => `${base}${filename}`),
        }),
    ],
});
