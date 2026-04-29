// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import sitemap from "@astrojs/sitemap";
import starlightTypeDoc, { typeDocSidebarGroup } from "starlight-typedoc";
import { slintStarlightFaviconHead } from "@slint/common-files/src/utils/starlight-favicon-head";
import { starlightExpandAllSidebarGroups } from "@slint/common-files/src/utils/starlight-expand-all-sidebar-groups";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
    slintStarlightMarkdownRehypeExternalLinksOnly,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";
import {
    NODE_DOCS_BASE_PATH,
    NODE_DOCS_BASE_URL,
    nodeDocsPublicAsset,
} from "./src/node-site-config.mjs";

const _nodeOrigin = String(NODE_DOCS_BASE_URL).replace(/\/+$/, "");
const _nodeAtRoot = NODE_DOCS_BASE_PATH === "/";
/** Canonical URL and optional `base` (same pattern as `docs/astro/astro.config.mjs`). */
const _nodeSite = _nodeAtRoot
    ? _nodeOrigin
    : `${_nodeOrigin}${NODE_DOCS_BASE_PATH.replace(/\/*$/, "/")}`;
const _nodeBase = _nodeAtRoot
    ? undefined
    : NODE_DOCS_BASE_PATH.replace(/\/*$/, "/");

export default defineConfig({
    site: _nodeSite,
    ...(_nodeBase ? { base: _nodeBase } : {}),
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: slintStarlightMarkdownRehypeExternalLinksOnly(),
    integrations: [
        sitemap(),
        starlight({
            title: "Slint Node.js API",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: [
                "@slint/common-files/src/styles/starlight-slint-custom.css",
                "@slint/common-files/src/styles/starlight-slint-theme.css",
            ],
            favicon: "favicon.svg",
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/HeaderNodeDocs.astro",
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
                slintStarlightLinksValidatorPlugin({
                    exclude: ({ link }) => {
                        const p = (link.split("?")[0] ?? "").trim();
                        return (
                            p.startsWith("/#") || p.startsWith("/thirdparty/")
                        );
                    },
                }),
                starlightExpandAllSidebarGroups(),
            ],
            social: slintStarlightSocial,
            head: slintStarlightFaviconHead(nodeDocsPublicAsset),
            sidebar: [
                { label: "Overview", slug: "index" },
                typeDocSidebarGroup,
            ],
        }),
    ],
});
