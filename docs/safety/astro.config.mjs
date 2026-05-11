// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import mermaid from "astro-mermaid";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
    slintStarlightMarkdownRehypeExternalLinksOnly,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";

// https://astro.build/config
export default defineConfig({
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: slintStarlightMarkdownRehypeExternalLinksOnly(),
    integrations: [
        mermaid(),
        starlight({
            title: "Slint SC Safety Manual",
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [slintStarlightLinksValidatorPlugin()],
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
            ],
        }),
    ],
});
