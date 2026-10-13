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
                Footer: "./src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
                Head: "@slint/common-files/src/components/Head.astro",
                MarkdownContent: "@slint/common-files/src/components/MarkdownContent.astro",
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
                                label: "Get Started",
                                items: [
                                    {
                                        label: "Prerequisites",
                                        slug: "user-manual/get-started/prerequisites",
                                    },
                                    {
                                        label: "Installation",
                                        slug: "user-manual/get-started/installation",
                                    },
                                    {
                                        label: "Compiling .slint Files",
                                        slug: "user-manual/get-started/compiling",
                                    },
                                    {
                                        label: "Using the Generated Code",
                                        slug: "user-manual/get-started/using-generated-code",
                                    },
                                ],
                            },
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
                                slug: "user-manual/compiler",
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
                        label: "Safety Plan",
                        link: "/safety-plan/",
                        items: [
                            {
                                label: "Overview",
                                slug: "safety-plan",
                            },
                            {
                                label: "Scope",
                                items: [
                                    {
                                        label: "Standards Compliance",
                                        slug: "safety-plan/standards-compliance",
                                    },
                                    {
                                        label: "Safety Management",
                                        slug: "safety-plan/safety-management",
                                    },
                                ],
                            },
                            {
                                label: "Lifecycle",
                                items: [
                                    {
                                        label: "Lifecycle",
                                        slug: "safety-plan/lifecycle",
                                    },
                                    {
                                        label: "Development Phases",
                                        slug: "safety-plan/development-phases",
                                    },
                                    {
                                        label: "Release and Field Monitoring",
                                        slug: "safety-plan/release",
                                    },
                                    {
                                        label: "Deliverables",
                                        slug: "safety-plan/deliverables",
                                    },
                                ],
                            },
                            {
                                label: "Design",
                                items: [
                                    {
                                        label: "Architecture Design",
                                        slug: "safety-plan/architecture",
                                    },
                                ],
                            },
                            {
                                label: "Process",
                                items: [
                                    {
                                        label: "Development Process",
                                        slug: "safety-plan/development-process",
                                    },
                                    {
                                        label: "Coding Standards",
                                        slug: "safety-plan/coding-standards",
                                    },
                                ],
                            },
                            {
                                label: "Verification",
                                items: [
                                    {
                                        label: "Verification",
                                        slug: "safety-plan/verification",
                                    },
                                    {
                                        label: "Test Suites",
                                        slug: "safety-plan/test-suites",
                                    },
                                    {
                                        label: "Coverage Criteria",
                                        slug: "safety-plan/test-coverage",
                                    },
                                    {
                                        label: "Coverage Tool Verification",
                                        slug: "safety-plan/slint-coverage",
                                    },
                                ],
                            },
                        ],
                    },
                    {
                        label: "Tool Evaluation Report",
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
                        ],
                    },
                    {
                        label: "Runtime Safety Documentation",
                        link: "/runtime/",
                        items: [
                            {
                                label: "Overview",
                                slug: "runtime",
                            },
                            {
                                label: "Assumed Safety Requirements",
                                slug: "runtime/assumed-safety-requirements",
                            },
                            {
                                label: "Safety Analysis",
                                slug: "runtime/safety-analysis",
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
                                label: "Tool Qualification",
                                slug: "qualification-report/tool-qualification",
                            },
                            {
                                label: "Evidence",
                                items: [
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
                        ],
                    },
                    {
                        label: "Norm Mapping",
                        link: "/norm-mapping/",
                        items: [
                            { label: "Overview", slug: "norm-mapping" },
                            {
                                label: "ISO 26262-2",
                                slug: "norm-mapping/iso-26262-2",
                            },
                            {
                                label: "ISO 26262-6",
                                slug: "norm-mapping/iso-26262-6",
                            },
                            {
                                label: "ISO 26262-6 Method Tables",
                                slug: "norm-mapping/iso-26262-6-methods",
                            },
                            {
                                label: "ISO 26262-8",
                                slug: "norm-mapping/iso-26262-8",
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
