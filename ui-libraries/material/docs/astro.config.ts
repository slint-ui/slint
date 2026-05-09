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
import path from "node:path";
import { fileURLToPath } from "node:url";

import sitemap from "@astrojs/sitemap";
import tailwind from "@astrojs/tailwind";
import mdx from "@astrojs/mdx";
import partytown from "@astrojs/partytown";
import compress from "astro-compress";
import type { AstroIntegration } from "astro";

import astrowind from "./vendor/integration";

import {
    readingTimeRemarkPlugin,
    responsiveTablesRehypePlugin,
    lazyImagesRehypePlugin,
} from "./src/utils/frontmatter";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const hasExternalScripts = false;
const whenExternalScripts = (
    items: (() => AstroIntegration) | (() => AstroIntegration)[] = [],
) =>
    hasExternalScripts
        ? Array.isArray(items)
            ? items.map((item) => item())
            : [items()]
        : [];

// https://astro.build/config
export default defineConfig({
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: {
        remarkPlugins: [readingTimeRemarkPlugin],
        rehypePlugins: [
            responsiveTablesRehypePlugin,
            lazyImagesRehypePlugin,
            rehypeExternalLinksSlint,
        ],
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
                "./src/assets/styles/starlight-material-supplement.css",
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
                    autogenerate: { directory: "components" },
                },
            ],
            plugins: [
                slintStarlightLinksValidatorPlugin({
                    exclude: ["/zip/**"],
                }),
            ],
            social: slintStarlightSocial,
            favicon: "favicon.svg",
            head: slintStarlightFaviconHead((filename) => `/${filename}`),
        }),
        tailwind({
            applyBaseStyles: false,
        }),
        sitemap(),
        mdx(),

        ...whenExternalScripts(() =>
            partytown({
                config: { forward: ["dataLayer.push"] },
            }),
        ),

        compress({
            CSS: true,
            HTML: {
                "html-minifier-terser": {
                    removeAttributeQuotes: false,
                },
            },
            Image: false,
            JavaScript: true,
            SVG: false,
            Logger: 1,
        }),

        astrowind({
            config: "./src/config.yaml",
        }),
    ],
    image: {
        domains: ["cdn.pixabay.com"],
    },
    vite: {
        resolve: {
            alias: {
                "~": path.resolve(__dirname, "./src"),
            },
        },
    },
});
