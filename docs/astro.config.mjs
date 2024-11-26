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
            title: "Docs",
            logo: {
                light: "./src/assets/slint-logo-simple-light.webp",
                dark: "./src/assets/slint-logo-simple-dark.webp",
            },
            customCss: ["./src/styles/custom.css"],
            lastUpdated: true,
            components: {
                Footer: "./src/components/Footer.astro",
                Header: "./src/components/Header.astro",
                Banner: "./src/components/Banner.astro",
            },
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
                    label: "Home",
                    slug: "index",
                },
                {
                    label: "Getting Started",
                    slug: "getting-started/intro",
                },
                {
                    label: "Guide",
                    collapsed: true,
                    items: [
                        { label: "Introduction", slug: "guide/intro" },
                        {
                            label: "Slint Language",
                            items: [
                                {
                                    label: "Basics",
                                    slug: "guide/language/basics",
                                },
                                {
                                    label: "Syntax",
                                    slug: "guide/language/syntax",
                                },
                                {
                                    label: "Types",
                                    slug: "guide/language/types",
                                },
                                {
                                    label: "Positioning & Layouts",
                                    slug: "guide/language/positioning-and-layouts",
                                }
                            ],
                        },
                        {
                            label: "App Development",
                            items: [
                                "guide/development/debugging_techniques",
                                "guide/development/localization",
                                "guide/development/fonts",
                            ],
                        },
                    ],
                },
                {
                    label: "Reference",
                    collapsed: true,
                    items: [
                        {
                            label: "Common details",
                            slug: "reference/overview",
                        },
                        {
                            label: "Basics",
                            items: [
                                "reference/builtins/builtinfunctions",
                                "reference/builtins/colors",
                                "reference/builtins/math",
                            ],
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
                        {
                            label: "Std-Widgets",
                            collapsed: true,
                            items: [
                                "reference/std-widgets/overview",
                                "reference/std-widgets/style",
                                {
                                    label: "UI Widgets",
                                    items: [
                                        "reference/std-widgets/aboutslint",
                                        "reference/std-widgets/button",
                                        "reference/std-widgets/checkbox",
                                        "reference/std-widgets/combobox",
                                        "reference/std-widgets/datepicker",
                                        "reference/std-widgets/lineedit",
                                        "reference/std-widgets/listview",
                                        "reference/std-widgets/progressindicator",
                                        "reference/std-widgets/scrollview",
                                        "reference/std-widgets/slider",
                                        "reference/std-widgets/spinbox",
                                        "reference/std-widgets/spinner",
                                        "reference/std-widgets/standardbutton",
                                        "reference/std-widgets/standardlistview",
                                        "reference/std-widgets/standardtableview",
                                        "reference/std-widgets/switch",
                                        "reference/std-widgets/tabwidget",
                                        "reference/std-widgets/textedit",
                                        "reference/std-widgets/timepicker",
                                    ],
                                },
                                {
                                    label: "Layout Widgets",
                                    items: [
                                        "reference/std-widgets/gridbox",
                                        "reference/std-widgets/groupbox",
                                        "reference/std-widgets/horizontalbox",
                                        "reference/std-widgets/verticalbox",
                                    ],
                                },
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
                    label: "Platforms",
                    collapsed: true,
                    autogenerate: { directory: "platforms" },
                },
                {
                    label: "Language Integrations",
                    collapsed: false,
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
            ],
        }),
    ],
});
