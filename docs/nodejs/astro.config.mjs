// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLlmsTxt from "starlight-llms-txt";
import sitemap from "@astrojs/sitemap";
import starlightTypeDoc from "starlight-typedoc";
import { slintStarlightFaviconHead } from "@slint/common-files/src/utils/starlight-favicon-head";
import { starlightExpandAllSidebarGroups } from "@slint/common-files/src/utils/starlight-expand-all-sidebar-groups";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
    slintStarlightMarkdownRehypeExternalLinksOnly,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";
import { THIRDPARTY_MD_LINK } from "@slint/common-files/src/utils/thirdparty.ts";
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

// Version-correct URLs to the sibling docs' llms.txt (see docs/astro for the
// rationale: the deploy rewrites "/<version>/docs" -> "/latest/docs" + host for
// the Cloudflare "latest" copy).
const _docsRoot = `${_nodeOrigin}${NODE_DOCS_BASE_PATH}`.replace(/node\/$/, "");
const siblingLlms = (/** @type {string} */ lang) =>
    `${_docsRoot}${lang}/llms.txt`;

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
                starlightLlmsTxt({
                    projectName: "Slint for JavaScript & TypeScript",
                    description:
                        "The JavaScript and TypeScript (Node.js) API documentation for Slint, a declarative GUI toolkit. Covers using `.slint` user interfaces from Node.js and TypeScript.",
                    optionalLinks: [
                        {
                            label: "Slint language docs (llms.txt)",
                            url: siblingLlms("slint"),
                            description:
                                "the .slint language, elements, and widgets",
                        },
                        {
                            label: "Slint C++ API (llms.txt)",
                            url: siblingLlms("cpp"),
                        },
                        {
                            label: "Slint Python API (llms.txt)",
                            url: siblingLlms("python"),
                        },
                        {
                            label: "Slint website",
                            url: "https://slint.dev",
                        },
                        {
                            label: "Slint on GitHub",
                            url: "https://github.com/slint-ui/slint",
                        },
                    ],
                    customSelectors: { all: ["a.sl-anchor-link"] },
                }),
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
                            p.startsWith("/#") ||
                            // The Third-Party Licenses page links to its own raw
                            // markdown sibling (served by the [...slug].md.ts
                            // endpoint). The relative form resolves correctly
                            // under any deployment base, but the validator only
                            // sees it as a relative link.
                            p === THIRDPARTY_MD_LINK ||
                            // starlight-typedoc deletes every subdirectory README.md but
                            // typedoc-plugin-markdown still emits a "Namespaces" link to
                            // the deleted file in the parent README. The namespace's
                            // type-alias sub-pages and the language variable page at
                            // /api/variables/language/ cover the same content.
                            p.endsWith(
                                "/api/slint-ui/namespaces/language/readme/",
                            )
                        );
                    },
                }),
                starlightExpandAllSidebarGroups(),
            ],
            social: slintStarlightSocial,
            head: slintStarlightFaviconHead(nodeDocsPublicAsset),
            // starlight-typedoc's auto-sidebar drops the "Namespaces" group because
            // typedoc-plugin-markdown nests namespaces under the parent module on disk
            // (`api/slint-ui/namespaces/…`), not at `api/namespaces/…` where the
            // auto-sidebar looks. We define the API sidebar manually so the `language`
            // variable expands to its struct/enum types.
            sidebar: [
                { label: "Overview", slug: "index" },
                {
                    label: "API",
                    items: [
                        {
                            label: "Classes",
                            collapsed: true,
                            items: [
                                { autogenerate: { directory: "api/classes" } },
                            ],
                        },
                        {
                            label: "Enumerations",
                            collapsed: true,
                            items: [
                                {
                                    autogenerate: {
                                        directory: "api/enumerations",
                                    },
                                },
                            ],
                        },
                        {
                            label: "Functions",
                            collapsed: true,
                            items: [
                                {
                                    autogenerate: {
                                        directory: "api/functions",
                                    },
                                },
                            ],
                        },
                        {
                            label: "Interfaces",
                            collapsed: true,
                            items: [
                                {
                                    autogenerate: {
                                        directory: "api/interfaces",
                                    },
                                },
                            ],
                        },
                        {
                            label: "Variables",
                            collapsed: true,
                            items: [
                                {
                                    label: "language",
                                    collapsed: true,
                                    items: [
                                        {
                                            label: "Overview",
                                            link: "/api/variables/language/",
                                        },
                                        {
                                            autogenerate: {
                                                directory:
                                                    "api/slint-ui/namespaces/language/type-aliases",
                                            },
                                        },
                                    ],
                                },
                            ],
                        },
                    ],
                },
                { autogenerate: { directory: "generated" } },
            ],
        }),
    ],
});
