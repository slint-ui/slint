// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeMermaid from "rehype-mermaid";
import addMermaidClass from "./src/utils/add-mermaid-classnames";
import rehypeExternalLinks from "rehype-external-links";

// https://astro.build/config
export default defineConfig({
    site: "https://snapshots.slint.dev/master/docs/slint/",
    base: "/master/docs/slint",
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
            addMermaidClass,
            rehypeMermaid,
        ],
    },
    integrations: [
        starlight({
            title: "Slint 1.9.0",
            logo: {
                light: "./src/assets/slint-logo-small-light.svg",
                dark: "./src/assets/slint-logo-small-dark.svg",
            },
            customCss: ["./src/styles/custom.css"],
            plugins: [
                starlightLinksValidator({
                    errorOnRelativeLinks: false,
                    errorOnLocalLinks: false,
                }),
            ],
            social: {
                github: "https://github.com/slint-ui/slint",
                "x.com": "https://x.com/slint_ui",
                linkedin: "https://www.linkedin.com/company/slint-ui/",
                mastodon: "https://fosstodon.org/@slint",
            },
            favicon: "./src/assets/favicon.svg",
            sidebar: [
                {
                    slug: "index",
                },
                {
                    label: "Getting Started",
                    slug: "getting-started/intro",
                },
                {
                    label: "Concepts",
                    collapsed: true,
                    items: [
                        { label: "Introduction", slug: "concepts/intro" },
                        {
                            label: "Slint Language",
                            items: [
                                {
                                    label: "Basics",
                                    slug: "concepts/language/basics",
                                },
                                {
                                    label: "Syntax",
                                    slug: "concepts/language/syntax",
                                },
                                {
                                    label: "Types",
                                    slug: "concepts/language/types",
                                },
                            ],
                        },
                        {
                            label: "App Development",
                            items: [
                                "concepts/development/debugging_techniques",
                                "concepts/development/localization",
                                "concepts/development/fonts",
                            ],
                        },
                    ],
                },
                {
                    label: "Tutorial",
                    collapsed: true,
                    items: [
                        { label: "Introduction", slug: "tutorial/quickstart" },

                        {
                            label: "Getting Started",
                            slug: "tutorial/getting_started",
                        },
                        { label: "Memory Tile", slug: "tutorial/memory_tile" },
                        {
                            label: "Polishing The Tile",
                            slug: "tutorial/polishing_the_tile",
                        },
                        {
                            label: "From One To Multiple Tiles",
                            slug: "tutorial/from_one_to_multiple_tiles",
                        },
                        {
                            label: "Creating The Tiles From Code",
                            slug: "tutorial/creating_the_tiles",
                        },
                        { label: "Game Logic", slug: "tutorial/game_logic" },
                        {
                            label: "Running In A Browser",
                            slug: "tutorial/running_in_a_browser",
                        },
                        {
                            label: "Ideas For The Reader",
                            slug: "tutorial/ideas_for_the_reader",
                        },
                        { label: "Conclusion", slug: "tutorial/conclusion" },
                    ],
                },
                {
                    label: "Std-Widgets",
                    collapsed: true,
                    items: [
                        "std-widgets/overview",
                        "std-widgets/style",
                        {
                            label: "UI Widgets",
                            items: [
                                "std-widgets/aboutslint",
                                "std-widgets/button",
                                "std-widgets/checkbox",
                                "std-widgets/combobox",
                                "std-widgets/datepicker",
                                "std-widgets/lineedit",
                                "std-widgets/listview",
                                "std-widgets/progressindicator",
                                "std-widgets/scrollview",
                                "std-widgets/slider",
                                "std-widgets/spinbox",
                                "std-widgets/spinner",
                                "std-widgets/standardbutton",
                                "std-widgets/standardlistview",
                                "std-widgets/standardtableview",
                                "std-widgets/switch",
                                "std-widgets/tabwidget",
                                "std-widgets/textedit",
                                "std-widgets/timepicker",
                            ],
                        },
                        {
                            label: "Layout Widgets",
                            items: [
                                "std-widgets/gridbox",
                                "std-widgets/groupbox",
                                "std-widgets/horizontalbox",
                                "std-widgets/verticalbox",
                            ],
                        },
                    ],
                },
                {
                    label: "Language Integrations",
                    collapsed: true,
                    items: [
                        {
                            label: "C++ ↗",
                            link: "https://docs.slint.dev/latest/docs/cpp/",
                            attrs: { target: "_blank" },
                        },
                        {
                            label: "Python ↗",
                            badge: { text: "beta", variant: "caution" },
                            link: "https://pypi.org/project/slint/",
                            attrs: { target: "_blank" },
                        },
                        {
                            label: "Rust ↗",
                            link: "https://docs.slint.dev/latest/docs/rust/slint/",
                            attrs: { target: "_blank" },
                        },
                        {
                            label: "TypeScript ↗",
                            link: "https://docs.slint.dev/latest/docs/node/",
                            attrs: { target: "_blank" },
                        },
                    ],
                },
                {
                    label: "Reference",
                    collapsed: true,
                    items: [
                        {
                            label: "Overview",
                            slug: "reference/overview",
                        },
                        {
                            label: "Builtin reference",
                            autogenerate: { directory: "reference/builtins" },
                        },
                        {
                            label: "Elements",
                            autogenerate: { directory: "reference/elements" },
                        },
                        {
                            label: "Gestures & Keyboard",
                            autogenerate: { directory: "reference/gestures" },
                        },
                        {
                            label: "Layouts",
                            autogenerate: { directory: "reference/layouts" },
                        },
                        {
                            label: "Window",
                            autogenerate: { directory: "reference/window" },
                        },
                    ],
                },
                {
                    label: "FAQ",
                    slug: "faq",
                },
                {
                    label: "SlintPad ↗",
                    link: "https://slintpad.com/",
                    attrs: { target: '_blank' },
                },
                {
                    label: "Slint Website ↗",
                    link: "https://slint.dev",
                    attrs: { target: '_blank' },
                }
            ],
        }),
    ],
});
