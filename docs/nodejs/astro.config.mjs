// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import starlightTypeDoc, { typeDocSidebarGroup } from "starlight-typedoc";
import rehypeExternalLinks from "rehype-external-links";
import { nodeDocsPublicAsset } from "./src/node-site-config.mjs";
import sitemap from '@astrojs/sitemap';

/**
 * Starlight plugin: starlight-typedoc defaults nested API groups to collapsed; force every
 * sidebar group (and autogenerate subgroup) to start expanded.
 */
function starlightExpandAllSidebarGroups() {
    return {
        name: "starlight-expand-all-sidebar-groups",
        hooks: {
            "config:setup"({ config, updateConfig }) {
                const { sidebar } = config;
                if (!Array.isArray(sidebar)) return;

                function expandEntries(entries) {
                    return entries.map((entry) => expandEntry(entry));
                }

                function expandEntry(entry) {
                    if (typeof entry === "string") return entry;
                    if (!entry || typeof entry !== "object") return entry;

                    const out = { ...entry };
                    if ("collapsed" in out) out.collapsed = false;
                    if (
                        out.autogenerate &&
                        typeof out.autogenerate === "object" &&
                        !Array.isArray(out.autogenerate)
                    ) {
                        out.autogenerate = { ...out.autogenerate, collapsed: false };
                    }
                    if (Array.isArray(out.items)) {
                        out.items = expandEntries(out.items);
                    }
                    return out;
                }

                updateConfig({ sidebar: expandEntries(sidebar) });
            },
        },
    };
}

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
        sitemap(),
        starlight({
            title: "Slint Node.js API",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: ["./src/styles/custom.css", "./src/styles/theme.css"],
            favicon: "favicon.svg",
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "./src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [
                starlightTypeDoc({
                    entryPoints: ["../../api/node/typescript/index.ts"],
                    tsconfig: "../../api/node/tsconfig.json",
                    sidebar: { label: "API" },
                    typeDoc: {
                        hideGenerator: true,
                        gitRevision: "master",
                    },
                }),
                starlightLinksValidator({
                    errorOnLocalLinks: false,
                    // Generated HTML in `public/thirdparty/`, not a Starlight docs route.
                    exclude: ["/thirdparty/**"],
                }),
                starlightExpandAllSidebarGroups(),
            ],
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/slint-ui/slint",
                },
            ],
            head: [
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/svg+xml",
                        href: nodeDocsPublicAsset("favicon.svg"),
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/png",
                        sizes: "32x32",
                        href: nodeDocsPublicAsset("favicon-32x32.png"),
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/png",
                        sizes: "16x16",
                        href: nodeDocsPublicAsset("favicon-16x16.png"),
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "icon",
                        type: "image/x-icon",
                        href: nodeDocsPublicAsset("favicon.ico"),
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "mask-icon",
                        href: nodeDocsPublicAsset("favicon.svg"),
                        color: "#8D46E7",
                    },
                },
                {
                    tag: "link",
                    attrs: {
                        rel: "apple-touch-icon",
                        sizes: "180x180",
                        href: nodeDocsPublicAsset("apple-touch-icon.png"),
                    },
                },
            ],
            sidebar: [
                { label: "Overview", slug: "index" },
                typeDocSidebarGroup,
            ],
        }),
    ],
});
