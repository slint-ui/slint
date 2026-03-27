// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeExternalLinks from "rehype-external-links";

// https://astro.build/config
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
        starlight({
            title: "Slint SC Safety Manual",
            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/Header.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
            },
            plugins: [
                starlightLinksValidator({
                    errorOnLocalLinks: false,
                }),
            ],
            sidebar: [
                { label: "Overview", slug: "index" },
                { label: "Safety Policy", slug: "safety-policy" },
                {
                    label: "Requirements",
                    items: [
                        {
                            label: "Overview",
                            slug: "requirements",
                        },
                        {
                            label: "Safe Rust Coding Standards",
                            slug: "requirements/coding-standards",
                        },
                        {
                            label: "Static Memory Allocation",
                            slug: "requirements/memory-allocation",
                        },
                        {
                            label: "Bounded Execution Time",
                            slug: "requirements/bounded-execution",
                        },
                        {
                            label: "State Machine Determinism",
                            slug: "requirements/state-machine",
                        },
                        {
                            label: "Resource Fallback",
                            slug: "requirements/resource-fallback",
                        },
                        {
                            label: "Code Generation",
                            slug: "requirements/code-generation",
                        },
                        {
                            label: "Test Coverage",
                            slug: "requirements/test-coverage",
                        },
                        {
                            label: "Separation of Concerns",
                            slug: "requirements/separation-of-concerns",
                        },
                        {
                            label: "Concurrency Control",
                            slug: "requirements/concurrency-control",
                        },
                    ],
                },
                { label: "System Components", slug: "system-components" },
                { label: "Development Cycle", slug: "development-cycle" },
                {
                    label: "Qualification Plan",
                    items: [
                        {
                            label: "Overview",
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
                            label: "Validation",
                            slug: "qualification-plan/validation",
                        },
                    ],
                },
            ],
        }),
    ],
});
