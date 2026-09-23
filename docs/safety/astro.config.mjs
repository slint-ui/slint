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
                        label: "User Manual",
                        link: "/user-manual/",
                        items: [
                            { label: "Overview", slug: "user-manual" },
                            {
                                label: "Known Problems",
                                slug: "user-manual/known-problems",
                            },
                            {
                                label: "Coverage of Slint Code",
                                slug: "user-manual/slint-coverage",
                            },
                            {
                                label: "Slint Compiler",
                                items: [
                                    {
                                        label: "Constraints",
                                        slug: "user-manual/compiler/constraints",
                                    },
                                ],
                            },
                            {
                                label: "slint-sc Runtime",
                                items: [
                                    {
                                        label: "Constraints",
                                        slug: "user-manual/runtime/constraints",
                                    },
                                ],
                            },
                        ],
                    },
                    {
                        label: "Reference",
                        link: "/reference/",
                        items: [
                            { label: "Overview", slug: "reference" },
                            {
                                label: "Language Specification",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Introduction",
                                        slug: "reference/language",
                                    },
                                    {
                                        label: "Source Files",
                                        slug: "reference/language/source-files",
                                    },
                                    {
                                        label: "Lexical Structure",
                                        slug: "reference/language/lexical-structure",
                                    },
                                    {
                                        label: "File Structure",
                                        slug: "reference/language/file-structure",
                                    },
                                    {
                                        label: "Name Resolution",
                                        slug: "reference/language/name-resolution",
                                    },
                                    {
                                        label: "Imports",
                                        slug: "reference/language/imports",
                                    },
                                    {
                                        label: "Exports",
                                        slug: "reference/language/exports",
                                    },
                                    {
                                        label: "Properties",
                                        slug: "reference/language/properties",
                                    },
                                    {
                                        label: "Bindings",
                                        slug: "reference/language/bindings",
                                    },
                                    {
                                        label: "Expressions",
                                        slug: "reference/language/expressions",
                                    },
                                    {
                                        label: "Operators",
                                        slug: "reference/language/operators",
                                    },
                                    {
                                        label: "Callbacks",
                                        slug: "reference/language/callbacks",
                                    },
                                    {
                                        label: "Structs and Enums",
                                        slug: "reference/language/structs-and-enums",
                                    },
                                    {
                                        label: "Geometry",
                                        slug: "reference/language/geometry",
                                    },
                                    {
                                        label: "States and Transitions",
                                        slug: "reference/language/states-and-transitions",
                                    },
                                ],
                            },
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
                    {
                        label: "Qualification Plan",
                        link: "/qualification-plan/",
                        items: [
                            {
                                label: "Overview",
                                slug: "qualification-plan",
                            },
                            {
                                label: "Scope",
                                items: [
                                    {
                                        label: "Standards Compliance",
                                        slug: "qualification-plan/standards-compliance",
                                    },
                                    {
                                        label: "Safety Policy",
                                        slug: "qualification-plan/safety-policy",
                                    },
                                ],
                            },
                            {
                                label: "Design",
                                items: [
                                    {
                                        label: "Architecture Design",
                                        slug: "qualification-plan/architecture",
                                    },
                                ],
                            },
                            {
                                label: "Process",
                                items: [
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
                                ],
                            },
                            {
                                label: "Verification",
                                items: [
                                    {
                                        label: "Verification",
                                        slug: "qualification-plan/verification",
                                    },
                                    {
                                        label: "Test Suites",
                                        slug: "qualification-plan/test-suites",
                                    },
                                    {
                                        label: "Coverage Criteria",
                                        slug: "qualification-plan/test-coverage",
                                    },
                                    {
                                        label: "Coverage Tool Verification",
                                        slug: "qualification-plan/slint-coverage",
                                    },
                                ],
                            },
                        ],
                    },
                    {
                        label: "Evaluation Report",
                        link: "/evaluation-report/",
                        items: [
                            {
                                label: "Overview",
                                slug: "evaluation-report",
                            },
                            {
                                label: "Use Cases",
                                slug: "evaluation-report/use-cases",
                            },
                            {
                                label: "Potential Errors",
                                slug: "evaluation-report/potential-errors",
                            },
                            {
                                label: "Tool Classification",
                                slug: "evaluation-report/tool-classification",
                            },
                            {
                                label: "Qualification Method",
                                slug: "evaluation-report/qualification-method",
                            },
                            {
                                label: "Safety Analysis",
                                slug: "evaluation-report/safety-analysis",
                            },
                        ],
                    },
                    {
                        label: "Qualification Report",
                        link: "/qualification-report/",
                        items: [
                            {
                                label: "Overview",
                                slug: "qualification-report",
                            },
                            {
                                label: "Test Results",
                                slug: "qualification-report/test-results",
                            },
                            {
                                label: "Traceability Matrix",
                                slug: "qualification-report/traceability-matrix",
                            },
                            {
                                label: "Test Coverage",
                                slug: "qualification-report/test-coverage",
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
