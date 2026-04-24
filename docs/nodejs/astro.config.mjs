// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import starlightTypeDoc, { typeDocSidebarGroup } from "starlight-typedoc";
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
                starlightTypeDoc({
                    entryPoints: ["../../api/node/typescript/index.ts"],
                    tsconfig: "../../api/node/tsconfig.json",
                    sidebar: { label: "API" },
                    typeDoc: {
                        hideGenerator: true,
                        // Avoid embedding the checkout’s HEAD in “Defined in:” URLs (breaks across machines/CI).
                        gitRevision: "master",
                    },
                }),
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
            sidebar: [
                { label: "Overview", slug: "index" },
                typeDocSidebarGroup,
            ],
        }),
    ],
});
