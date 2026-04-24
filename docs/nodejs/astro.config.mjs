// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeExternalLinks from "rehype-external-links";

// https://astro.build/config
// Production `site` / `base` are wired in PR4 (CI); local dev uses root URLs.
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
            title: "Slint Node.js API",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: ["./src/styles/custom.css", "./src/styles/theme.css"],
            favicon: "favicon.svg",
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
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
            ],
            sidebar: [{ label: "Overview", slug: "index" }],
        }),
    ],
});
