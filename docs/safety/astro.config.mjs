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
                    // The llvm-cov HTML report the Test Coverage chapter links
                    // into is a static asset under public/, not Starlight pages.
                    exclude: ["/coverage/**"],
                }),
            ],
            social: slintStarlightSocial,
            sidebar: [
                { label: "Slint SC Safety Manual", slug: "index" },
                { label: "Safety Policy", slug: "safety-policy" },
                {
                    label: "Requirements",
                    items: [
                        {
                            label: "ISO 26262 Requirements",
                            slug: "requirements",
                        },
                        {
                            label: "SR_SAFE_RUST_CODING_STANDARDS",
                            slug: "requirements/coding-standards",
                        },
                        {
                            label: "SR_STATIC_MEMORY_ALLOCATION",
                            slug: "requirements/memory-allocation",
                        },
                        {
                            label: "SR_BOUNDED_EXECUTION_TIME",
                            slug: "requirements/bounded-execution",
                        },
                        {
                            label: "SR_STATE_MACHINE_DETERMINISM",
                            slug: "requirements/state-machine",
                        },
                        {
                            label: "SR_RESOURCE_FALLBACK",
                            slug: "requirements/resource-fallback",
                        },
                        {
                            label: "SR_CODE_GENERATION",
                            slug: "requirements/code-generation",
                        },
                        {
                            label: "SR_TEST_COVERAGE",
                            slug: "requirements/test-coverage",
                        },
                        {
                            label: "SR_SEPARATION_OF_CONCERNS",
                            slug: "requirements/separation-of-concerns",
                        },
                        {
                            label: "SR_CONCURRENCY_CONTROL",
                            slug: "requirements/concurrency-control",
                        },
                    ],
                },
                { label: "Using Slint SC", slug: "using-slint-sc" },
                {
                    label: "Reference",
                    items: [
                        { label: "Overview", slug: "reference" },
                        {
                            label: "Generated Code",
                            slug: "reference/generated-code",
                        },
                        { label: "Rendering", slug: "reference/rendering" },
                        {
                            label: "Elements",
                            items: [
                                { label: "Rectangle", slug: "reference/rectangle" },
                                { label: "Window", slug: "reference/window" },
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
                                    label: "Numeric Types",
                                    slug: "reference/property-types/numeric-types",
                                },
                            ],
                        },
                    ],
                },
                { label: "Development Process", slug: "development-process" },
                { label: "Development Phases", slug: "development-phases" },
                {
                    label: "Qualification Plan",
                    items: [
                        {
                            label: "Qualification Plan",
                            slug: "qualification-plan",
                        },
                        {
                            label: "Failure Scenarios",
                            slug: "qualification-plan/failure-scenarios",
                        },
                        {
                            label: "Known Issues",
                            slug: "qualification-plan/known-issues",
                        },
                        {
                            label: "Test Cases",
                            slug: "qualification-plan/test-cases",
                        },
                        {
                            label: "Validation",
                            slug: "qualification-plan/validation",
                        },
                        {
                            label: "Traceability Matrix",
                            slug: "qualification-plan/traceability-matrix",
                        },
                        {
                            label: "Test Coverage",
                            slug: "qualification-plan/test-coverage",
                        },
                    ],
                },
                {
                    label: "Language Specification",
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
                        // The Imports chapter isn't in the SC subset yet (no
                        // `SC: true`); list it here once it joins.
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
                            label: "Geometry",
                            slug: "language/geometry",
                        },
                    ],
                },
            ],
        }),
    ],
});
