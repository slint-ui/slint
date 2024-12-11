// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeMermaid from "rehype-mermaid";
import addMermaidClass from "./src/utils/add-mermaid-classnames";
import rehypeExternalLinks from "rehype-external-links";
import starlightSidebarTopics from "starlight-sidebar-topics";

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
            title: "Slint Docs",
            logo: {
                light: "./src/assets/slint-logo-simple-light.webp",
                dark: "./src/assets/slint-logo-simple-dark.webp",
                replacesTitle: true,
            },
            customCss: ["./src/styles/custom.css", "./src/styles/theme.css"],

            components: {
                Footer: "./src/components/Footer.astro",
                Header: "./src/components/Header.astro",
                Banner: "./src/components/Banner.astro",
            },
            plugins: [
                starlightSidebarTopics([
                    {
                        label: "Guide",
                        link: "",
                        icon: "open-book",
                        items: [
                            { label: "Overview", slug: "index" },
                            {
                                label: "Tooling",
                                collapsed: true,
                                items: ["guide/tooling/vscode"],
                            },
                            {
                                label: "Language",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Concepts",
                                        items: [
                                            {
                                                label: "Slint Language",
                                                slug: "guide/language/concepts/slint-language",
                                            },
                                            {
                                                label: "Components and Properties",
                                                slug: "guide/language/concepts/component-property",
                                            },
                                            {
                                                label: "Reactivity",
                                                slug: "guide/language/concepts/reactivity",
                                            },
                                        ],
                                    },
                                    {
                                        label: "Coding",
                                        items: [
                                            {
                                                label: "The `.slint` File",
                                                slug: "guide/language/coding/file",
                                            },
                                            {
                                                label: "Properties",
                                                slug: "guide/language/coding/properties",
                                            },
                                            {
                                                label: "Expressions and Statements",
                                                slug: "guide/language/coding/expressions-and-statements",
                                            },
                                            {
                                                label: "Positioning & Layouts",
                                                slug: "guide/language/coding/positioning-and-layouts",
                                            },
                                            {
                                                label: "In App Communication",
                                                slug: "guide/language/coding/in-app-communication",
                                            },
                                            {
                                                label: "Globals",
                                                slug: "guide/language/coding/globals",
                                            },
                                            {
                                                label: "Repetition and Data Models",
                                                slug: "guide/language/coding/repetition-and-data-models",
                                            },
                                            {
                                                label: "Animations",
                                                slug: "guide/language/coding/animation",
                                            },
                                            {
                                                label: "States and Transitions",
                                                slug: "guide/language/coding/states",
                                            },
                                            {
                                                label: "Functions and Callbacks",
                                                slug: "guide/language/coding/functions-and-callbacks",
                                            },
                                            {
                                                label: "Name Resolution (Scope)",
                                                slug: "guide/language/coding/name-resolution",
                                            },
                                            {
                                                label: "Structs and Enums",
                                                slug: "guide/language/coding/structs-and-enums",
                                            },
                                        ],
                                    },
                                ],
                            },
                            {
                                label: "App Development",
                                collapsed: true,
                                items: [
                                    "guide/development/debugging_techniques",
                                    "guide/development/focus",
                                    "guide/development/translations",
                                    "guide/development/fonts",
                                    {
                                        label: "Custom Controls",
                                        slug: "guide/development/custom-controls",
                                    },
                                ],
                            },
                            {
                                label: "Platforms",
                                collapsed: true,
                                items: [
                                    "guide/platforms/desktop",
                                    "guide/platforms/embedded",
                                    "guide/platforms/mobile",
                                ],
                            },
                            {
                                label: "Backends and Renderers",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Overview",
                                        slug: "guide/backends-and-renderers/backends_and_renderers",
                                    },
                                    "guide/backends-and-renderers/backend_linuxkms",
                                    "guide/backends-and-renderers/backend_qt",
                                    "guide/backends-and-renderers/backend_winit",
                                ],
                            },
                        ],
                    },
                    {
                        label: "Reference",
                        link: "reference/primitive-types",
                        icon: "information",
                        items: [
                            {
                                label: "Primitive Types",
                                slug: "reference/primitive-types",
                            },
                            {
                                label: "Colors & Brushes",
                                slug: "reference/colors-and-brushes",
                            },
                            {
                                label: "Common Properties & Callbacks",
                                slug: "reference/common",
                            },
                            {
                                label: "Basic Elements",
                                collapsed: true,
                                autogenerate: {
                                    directory: "reference/elements",
                                },
                            },
                            {
                                label: "Gestures & Keyboard",
                                collapsed: true,
                                autogenerate: {
                                    directory: "reference/gestures",
                                },
                            },
                            {
                                label: "Layouts",
                                collapsed: true,
                                autogenerate: {
                                    directory: "reference/layouts",
                                },
                            },
                            {
                                label: "Window",
                                collapsed: true,
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
                            {
                                label: "Global Structs and Enums",
                                slug: "reference/global-structs-enums",
                            },
                            {
                                label: "Global Functions",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Math",
                                        slug: "reference/global-functions/math",
                                    },
                                    {
                                        label: "animation-tick() / debug()",
                                        slug: "reference/global-functions/builtinfunctions",
                                    },
                                ],
                            },
                        ],
                    },
                    {
                        label: "Tutorial",
                        link: "tutorial/quickstart",
                        icon: "seti:todo",
                        items: [
                            {
                                label: "Introduction",
                                slug: "tutorial/quickstart",
                            },

                            {
                                label: "Getting Started",
                                slug: "tutorial/getting_started",
                            },
                            {
                                label: "Memory Tile",
                                slug: "tutorial/memory_tile",
                            },
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
                            {
                                label: "Game Logic",
                                slug: "tutorial/game_logic",
                            },
                            {
                                label: "Running In A Browser",
                                slug: "tutorial/running_in_a_browser",
                            },
                            {
                                label: "Ideas For The Reader",
                                slug: "tutorial/ideas_for_the_reader",
                            },
                            {
                                label: "Conclusion",
                                slug: "tutorial/conclusion",
                            },
                        ],
                    },
                    {
                        label: "Language Integrations",
                        link: "language-integrations",
                        icon: "seti:html",
                        items: [
                            {
                                label: "C++ ↗",
                                link: "https://docs.slint.dev/latest/docs/cpp/",
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "Rust ↗",
                                link: "https://docs.slint.dev/latest/docs/rust/slint/",
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "TypeScript ↗",
                                badge: {
                                    text: "beta",
                                    variant: "caution",
                                },
                                link: "https://docs.slint.dev/latest/docs/node/",
                                attrs: { target: "_blank" },
                            },
                        ],
                    },
                ]),
                starlightLinksValidator({
                    errorOnLocalLinks: false,
                    exclude: ["**/index.mdx"],
                }),
            ],
            social: {
                github: "https://github.com/slint-ui/slint",
                "x.com": "https://x.com/slint_ui",
                linkedin: "https://www.linkedin.com/company/slint-ui/",
                mastodon: "https://fosstodon.org/@slint",
            },
            favicon: "favicon.svg",
        }),
    ],
});
