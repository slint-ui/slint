// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLlmsTxt from "starlight-llms-txt";
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
import rehypeSlsIds from "./src/rehype-sls-ids.mjs";

const _safetyOrigin = String(SAFETY_DOCS_BASE_URL).replace(/\/+$/, "");
const _safetyAtRoot = SAFETY_DOCS_BASE_PATH === "/";
const _safetySite = _safetyAtRoot
    ? _safetyOrigin
    : `${_safetyOrigin}${SAFETY_DOCS_BASE_PATH.replace(/\/*$/, "/")}`;
const _safetyBase = _safetyAtRoot
    ? undefined
    : SAFETY_DOCS_BASE_PATH.replace(/\/*$/, "/");

// https://astro.build/config
// Version-correct URL to the Slint language docs' llms.txt (see docs/astro for
// the rationale: the deploy rewrites "/<version>/docs" -> "/latest/docs" + host
// for the Cloudflare "latest" copy).
const _docsRoot = `${_safetyOrigin}${SAFETY_DOCS_BASE_PATH}`.replace(
    /safety\/$/,
    "",
);
const siblingLlms = (/** @type {string} */ lang) =>
    `${_docsRoot}${lang}/llms.txt`;

export default defineConfig({
    site: _safetySite,
    ...(_safetyBase ? { base: _safetyBase } : {}),
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: {
        rehypePlugins: [rehypeExternalLinksSlint, rehypeSlsIds],
    },
    integrations: [
        mermaid(),
        starlight({
            title: "Slint SC Safety Manual",
            customCss: [
                "@slint/common-files/src/styles/starlight-slint-custom.css",
                "@slint/common-files/src/styles/starlight-slint-theme.css",
                "./src/styles/sls-ids.css",
            ],
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [
                slintStarlightLinksValidatorPlugin({ errorOnRelativeLinks: false }),
                starlightLlmsTxt({
                    projectName: "Slint Safety",
                    description:
                        "Functional safety documentation for Slint, a declarative GUI toolkit, including safety-related guidance and processes.",
                    optionalLinks: [
                        {
                            label: "Slint language docs (llms.txt)",
                            url: siblingLlms("slint"),
                            description: "the .slint language, elements, and widgets",
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
                            label: "Elements",
                            items: [
                                {
                                    autogenerate: {
                                        directory:
                                            "reference/generated/elements",
                                    },
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
                        // {
                        //     label: "Imports",
                        //     slug: "language/imports",
                        // },
                        {
                            label: "Exports",
                            slug: "language/exports",
                        },
                    ],
                },
            ],
        }),
    ],
});
