// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeExternalLinks from "rehype-external-links";

// https://astro.build/config
export default defineConfig({
    trailingSlash: "always",
    markdown: {
        rehypePlugins: [
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
            title: "Slint Docs",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: ["./src/styles/custom.css", "./src/styles/theme.css"],

            components: {
                Footer: "./src/components/Footer.astro",
                Header: "./src/components/Header.astro",
                Banner: "./src/components/Banner.astro",
            },
            sidebar: [
                { label: "Overview", link: "overview" },
                { label: "Style", link: "style" },

                {
                    label: "Basic Widgets",
                    autogenerate: { directory: "basic-widgets" },
                },
            ],
            plugins: [
                starlightLinksValidator({
                    errorOnLocalLinks: false,
                }),
            ],
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/slint-ui/slint",
                },
                { icon: "x.com", label: "X", href: "https://x.com/slint_ui" },
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
            ],
            favicon: "favicon.svg",
        }),
    ],
});
