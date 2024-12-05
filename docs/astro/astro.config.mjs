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
            title: "Docs",
            logo: {
                light: "./src/assets/slint-logo-simple-light.webp",
                dark: "./src/assets/slint-logo-simple-dark.webp",
            },
            customCss: ["./src/styles/custom.css"],

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
                                items: [
                                    "guide/tooling/slintpad",
                                    "guide/tooling/vscode",
                                    "guide/tooling/live-editor",
                                ],
                            },
                            {
                                label: "Language",
                                items: [
                                    {
                                        label: "Core Concepts",
                                        items: [
                                            {
                                                label: "Components and Properties",
                                                slug: "guide/language/concepts/component-property",
                                            },
                                            {
                                                label: "Reactivity",
                                                slug: "guide/language/concepts/reactivity",
                                            },
                                            {
                                                label: "Purity",
                                                slug: "guide/language/concepts/purity",
                                            },
                                        ],
                                    },
                                    {
                                        label: "Syntax",
                                        items: [
                                            {
                                                label: "The `.slint` File",
                                                slug: "guide/language/syntax/file",
                                            },
                                            {
                                                label: "Basics",
                                                slug: "guide/language/syntax/basics",
                                            },
                                            {
                                                label: "Syntax",
                                                slug: "guide/language/syntax/syntax",
                                            },
                                            {
                                                label: "Positioning & Layouts",
                                                slug: "guide/language/syntax/positioning-and-layouts",
                                            },
                                            "guide/language/syntax/expressions",
                                            "guide/language/syntax/functions",
                                            "guide/language/syntax/callbacks",
                                            "guide/language/syntax/globals",
                                            "guide/language/syntax/modules",
                                            "guide/language/syntax/properties",
                                            "guide/language/syntax/statements",
                                        ],
                                    },
                                    {
                                        label: "Advanced",
                                        items: [
                                            "guide/language/advanced/animation",
                                            "guide/language/advanced/states",
                                            "guide/language/advanced/repetitions",
                                            "guide/language/advanced/container",
                                        ],
                                    },
                                ],
                            },
                            {
                                label: "App Development",
                                items: [
                                    "guide/development/debugging_techniques",
                                    "guide/development/localization",
                                    "guide/development/fonts",
                                    "guide/development/focus",
                                ],
                            },
                        ],
                    },
                    {
                        label: "Reference",
                        link: "reference/overview",
                        icon: "information",
                        items: [
                            {
                                label: "Common details",
                                slug: "reference/overview",
                            },
                            {
                                label: "Basics",
                                collapsed: true,
                                items: [
                                    "reference/builtins/types",
                                    "reference/builtins/type-conversions",
                                    "reference/builtins/builtinfunctions",
                                    "reference/builtins/colors",
                                    "reference/builtins/math",
                                    "reference/builtins/animations",
                                    "reference/builtins/key",
                                ],
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
                        label: "Platforms & Integrations",
                        link: "platforms",
                        icon: "seti:html",
                        items: [
                            {
                                label: "Platforms",
                                collapsed: false,
                                items: [
                                    "platforms/desktop",
                                    "platforms/embedded",
                                    "platforms/mobile",
                                ],
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
                            {
                                label: "Miscellaneous",
                                collapsed: false,
                                autogenerate: { directory: "platforms/misc" },
                            },
                        ],
                    },
                ]),
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
            favicon: "favicon.svg",
        }),
    ],
});
