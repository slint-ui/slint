// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeExternalLinks from "rehype-external-links";

import path from "node:path";
import { fileURLToPath } from "node:url";

import sitemap from "@astrojs/sitemap";
import tailwind from "@astrojs/tailwind";
import mdx from "@astrojs/mdx";
import partytown from "@astrojs/partytown";
import icon from "astro-icon";
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
    trailingSlash: "always",
    markdown: {
        remarkPlugins: [readingTimeRemarkPlugin],
        rehypePlugins: [
            responsiveTablesRehypePlugin,
            lazyImagesRehypePlugin,
            [
                rehypeExternalLinks,
                {
                    content: {
                        type: "text",
                        value: " ↗",
                    },
                    properties: {
                        target: "_blank",
                    },
                    rel: ["noopener"],
                },
            ],
        ],
    },
    integrations: [
        starlight({
            title: "Slint Material Components",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: [
                "./src/assets/styles/custom.css",
                "./src/assets/styles/theme.css",
            ],
            components: {
                Footer: "./src/components/Footer.astro",
                Header: "./src/components/Header.astro",
                Banner: "./src/components/Banner.astro",
            },
            sidebar: [
                { label: "Getting Started", link: "getting-started" },
                {
                    label: "Components",
                    autogenerate: { directory: "components" },
                },
            ],
            plugins: [starlightLinksValidator()],
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/slint-ui/material-components",
                },
                { icon: "x.com", label: "X", href: "https://x.com/slint_ui" },
                {
                    icon: "blueSky",
                    label: "Bluesky",
                    href: "https://bsky.app/profile/slint.dev",
                },
                {
                    icon: "linkedin",
                    label: "Linkedin",
                    href: "https://www.linkedin.com/company/slint-ui/",
                },
                {
                    icon: "mastodon",
                    label: "Mastodon",
                    href: "https://fosstodon.org/@slint",
                },
                {
                    icon: "youtube",
                    label: "YouTube",
                    href: "https://www.youtube.com/@slint-ui",
                },
            ],
            favicon: "favicon.svg",
            head: [
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/svg+xml",
                        href: "/favicon.svg",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/png",
                        sizes: "32x32",
                        href: "/favicon-32x32.png",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/png",
                        sizes: "16x16",
                        href: "/favicon-16x16.png",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/x-icon",
                        href: "/favicon.ico",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "mask-icon",
                        href: "/favicon.svg",
                        color: "#8D46E7",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "apple-touch-icon",
                        sizes: "180x180",
                        href: "/apple-touch-icon.png",
                    },
                },
            ],
        }),
        tailwind({
            applyBaseStyles: false,
        }),
        sitemap(),
        mdx(),
        icon({
            include: {
                tabler: ["*"],
                "flat-color-icons": [
                    "template",
                    "gallery",
                    "approval",
                    "document",
                    "advertising",
                    "currency-exchange",
                    "voice-presentation",
                    "business-contact",
                    "database",
                ],
            },
        }),

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
