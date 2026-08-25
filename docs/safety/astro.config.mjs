// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import mermaid from "astro-mermaid";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { rehypeExternalLinksSlint } from "@slint/common-files/src/utils/rehype-external-links-preset";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";
import {
    SAFETY_DOCS_BASE_URL,
    SAFETY_DOCS_BASE_PATH,
} from "./src/safety-site-config.mjs";
import rehypeSlsIds from "@slint/common-files/src/utils/rehype-sls-ids.mjs";
import remarkBaseLinks from "@slint/common-files/src/utils/remark-base-links.mjs";
import starlightSidebarTopics from "starlight-sidebar-topics";

const _safetyOrigin = String(SAFETY_DOCS_BASE_URL).replace(/\/+$/, "");
const _safetyAtRoot = SAFETY_DOCS_BASE_PATH === "/";
const _safetySite = _safetyAtRoot
    ? _safetyOrigin
    : `${_safetyOrigin}${SAFETY_DOCS_BASE_PATH.replace(/\/*$/, "/")}`;
const _safetyBase = _safetyAtRoot
    ? undefined
    : SAFETY_DOCS_BASE_PATH.replace(/\/*$/, "/");

// https://astro.build/config
export default defineConfig({
    site: _safetySite,
    ...(_safetyBase ? { base: _safetyBase } : {}),
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: {
        // Only SC-covered content reaches this site's generated reference, so
        // every paragraph of it carries a traceability id.
        remarkPlugins: [[remarkBaseLinks, { base: _safetyBase ?? "/" }]],
        rehypePlugins: [
            rehypeExternalLinksSlint,
            [rehypeSlsIds, { referenceRequiresIds: true }],
        ],
    },
    integrations: [
        mermaid(),
        starlight({
            title: "Slint SC Safety Manual",
            customCss: [
                "@slint/common-files/src/styles/starlight-slint-custom.css",
                "@slint/common-files/src/styles/starlight-slint-theme.css",
                "@slint/common-files/src/styles/sls-ids.css",
            ],
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [
                slintStarlightLinksValidatorPlugin({
                    errorOnRelativeLinks: true,
                    // Static assets under public/, not Starlight pages, and the
                    // built-in type pages, which only the Slint docs site has:
                    // its generated struct partials link to them and this build
                    // compiles them all (`generated-reference-markdown.ts`),
                    // even though no page of the manual renders one. Matched
                    // with a leading `**` because the links carry the base path
                    // the site is deployed under.
                    exclude: [
                        "**/coverage/**",
                        "**/api/**",
                        "**/property-types/builtin-enums/#*",
                        "**/property-types/builtin-structs/#*",
                    ],
                }),
                // One topic per document of the package. The site is a single
                // Starlight build; the topics are what make it read as a set,
                // each with its own URL prefix and its own sidebar.
                starlightSidebarTopics([
                    {
                        label: "Safety Manual",
                        link: "/safety-manual/",
                        items: [
                            { label: "Overview", slug: "safety-manual" },
                            {
                                label: "Known Problems",
                                slug: "safety-manual/known-problems",
                            },
                            {
                                label: "Slint Compiler",
                                items: [
                                    {
                                        label: "Constraints",
                                        slug: "safety-manual/compiler/constraints",
                                    },
                                ],
                            },
                            {
                                label: "slint-sc Runtime",
                                items: [
                                    {
                                        label: "Constraints",
                                        slug: "safety-manual/runtime/constraints",
                                    },
                                ],
                            },
                        ],
                    },
                    {
                        label: "Qualification Plan",
                        link: "/qualification-plan/",
                        items: [
                            { label: "Overview", slug: "qualification-plan" },
                            {
                                label: "Safety Policy",
                                slug: "qualification-plan/safety-policy",
                            },
                            {
                                label: "Architecture Design",
                                slug: "qualification-plan/architecture",
                            },
                            {
                                label: "Development Process",
                                slug: "qualification-plan/development-process",
                            },
                            {
                                label: "Development Phases",
                                slug: "qualification-plan/development-phases",
                            },
                            {
                                label: "Coding Standards",
                                slug: "qualification-plan/coding-standards",
                            },
                            {
                                label: "Test Suites",
                                slug: "qualification-plan/test-suites",
                            },
                            {
                                label: "Test Coverage",
                                slug: "qualification-plan/test-coverage",
                            },
                            {
                                label: "Verification",
                                slug: "qualification-plan/verification",
                            },
                            {
                                label: "Standards Compliance",
                                slug: "qualification-plan/standards-compliance",
                            },
                        ],
                    },
                    {
                        label: "Evaluation Report",
                        link: "/evaluation-report/use-cases/",
                        items: [
                            {
                                label: "Use Cases",
                                slug: "evaluation-report/use-cases",
                            },
                            {
                                label: "Potential Errors",
                                slug: "evaluation-report/potential-errors",
                            },
                            {
                                label: "Safety Analysis",
                                slug: "evaluation-report/safety-analysis",
                            },
                            {
                                label: "Tool Classification",
                                slug: "evaluation-report/tool-classification",
                            },
                            {
                                label: "Qualification Method",
                                slug: "evaluation-report/qualification-method",
                            },
                        ],
                    },
                    {
                        label: "Qualification Report",
                        link: "/qualification-report/traceability-matrix/",
                        items: [
                            {
                                label: "Traceability Matrix",
                                slug: "qualification-report/traceability-matrix",
                            },
                            {
                                label: "Test Coverage",
                                slug: "qualification-report/test-coverage",
                            },
                            {
                                label: "Test Results",
                                slug: "qualification-report/test-results",
                            },
                        ],
                    },
                    {
                        label: "Language Specification",
                        link: "/language/",
                        items: [
                            { label: "Introduction", slug: "language" },
                            {
                                label: "Source Files",
                                slug: "language/source-files",
                            },
                            {
                                label: "Lexical Structure",
                                slug: "language/lexical-structure",
                            },
                            {
                                label: "File Structure",
                                slug: "language/file-structure",
                            },
                            {
                                label: "Name Resolution",
                                slug: "language/name-resolution",
                            },
                            {
                                label: "Imports",
                                slug: "language/imports",
                            },
                            {
                                label: "Exports",
                                slug: "language/exports",
                            },
                            {
                                label: "Properties",
                                slug: "language/properties",
                            },
                            {
                                label: "Bindings",
                                slug: "language/bindings",
                            },
                            {
                                label: "Expressions",
                                slug: "language/expressions",
                            },
                            {
                                label: "Operators",
                                slug: "language/operators",
                            },
                            {
                                label: "Callbacks",
                                slug: "language/callbacks",
                            },
                            {
                                label: "Structs and Enums",
                                slug: "language/structs-and-enums",
                            },
                            {
                                label: "Geometry",
                                slug: "language/geometry",
                            },
                        ],
                    },
                    {
                        label: "API Reference",
                        link: "/reference/",
                        items: [
                            { label: "Overview", slug: "reference" },
                            {
                                label: "Generated Code",
                                slug: "reference/generated-code",
                            },
                            { label: "Rendering", slug: "reference/rendering" },
                            {
                                label: "Touch Input",
                                slug: "reference/input",
                            },
                            {
                                label: "Elements",
                                items: [
                                    {
                                        label: "Image",
                                        slug: "reference/image",
                                    },
                                    {
                                        label: "Rectangle",
                                        slug: "reference/rectangle",
                                    },
                                    {
                                        label: "TouchArea",
                                        slug: "reference/toucharea",
                                    },
                                    {
                                        label: "Window",
                                        slug: "reference/window",
                                    },
                                ],
                            },
                            {
                                label: "Property Types",
                                items: [
                                    {
                                        label: "Colors & Brushes",
                                        slug: "reference/property-types/colors-and-brushes",
                                    },
                                    {
                                        label: "Images",
                                        slug: "reference/property-types/images",
                                    },
                                    {
                                        label: "Numeric Types",
                                        slug: "reference/property-types/numeric-types",
                                    },
                                ],
                            },
                            {
                                // Directory form: `trailingSlash: "always"` would
                                // rewrite a link ending in `index.html` to `index/`.
                                label: "slint-sc Runtime API ↗",
                                link: "/api/slint_sc/",
                                attrs: { target: "_blank" },
                            },
                        ],
                    },
                ], {
                    // The landing page lists the documents and belongs to none
                    // of them.
                    exclude: ["/"],
                }),
            ],
            social: slintStarlightSocial,
        }),
    ],
});
