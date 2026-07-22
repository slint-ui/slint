// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import sitemap from "@astrojs/sitemap";
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
    PYTHON_DOCS_BASE_PATH,
    PYTHON_DOCS_BASE_URL,
    pythonDocsPublicAsset,
} from "./src/python-site-config.mjs";

const _pyOrigin = String(PYTHON_DOCS_BASE_URL).replace(/\/+$/, "");
const _pyAtRoot = PYTHON_DOCS_BASE_PATH === "/";
/** Canonical URL and optional `base` (same pattern as `docs/nodejs/astro.config.mjs`). */
const _pySite = _pyAtRoot
    ? _pyOrigin
    : `${_pyOrigin}${PYTHON_DOCS_BASE_PATH.replace(/\/*$/, "/")}`;
const _pyBase = _pyAtRoot
    ? undefined
    : PYTHON_DOCS_BASE_PATH.replace(/\/*$/, "/");

export default defineConfig({
    site: _pySite,
    ...(_pyBase ? { base: _pyBase } : {}),
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: slintStarlightMarkdownRehypeExternalLinksOnly(),
    integrations: [
        sitemap(),
        starlight({
            title: "Slint Python API",
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
                Header: "@slint/common-files/src/components/HeaderPythonDocs.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [
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
                            p === THIRDPARTY_MD_LINK
                        );
                    },
                }),
                starlightExpandAllSidebarGroups(),
            ],
            social: slintStarlightSocial,
            head: slintStarlightFaviconHead(pythonDocsPublicAsset),
            sidebar: [
                { label: "Overview", slug: "index" },
                {
                    label: "API",
                    items: [
                        {
                            label: "Classes",
                            items: [
                                { autogenerate: { directory: "api/classes" } },
                            ],
                        },
                        {
                            label: "Enumerations",
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
                            items: [
                                {
                                    autogenerate: {
                                        directory: "api/functions",
                                    },
                                },
                            ],
                        },
                        {
                            label: "Variables",
                            items: [
                                {
                                    autogenerate: {
                                        directory: "api/variables",
                                    },
                                },
                            ],
                        },
                        {
                            label: "language",
                            items: [
                                {
                                    label: "Classes",
                                    items: [
                                        {
                                            autogenerate: {
                                                directory:
                                                    "api/language/classes",
                                            },
                                        },
                                    ],
                                },
                                {
                                    label: "Enumerations",
                                    items: [
                                        {
                                            autogenerate: {
                                                directory:
                                                    "api/language/enumerations",
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
