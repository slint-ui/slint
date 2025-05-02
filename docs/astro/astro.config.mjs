// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeExternalLinks from "rehype-external-links";
import starlightSidebarTopics from "starlight-sidebar-topics";
import {
    BASE_PATH,
    BASE_URL,
    CPP_BASE_URL,
    RUST_SLINT_CRATE_URL,
    NODEJS_BASE_URL,
    PYTHON_BASE_URL,
} from "./src/utils/site-config";

// https://astro.build/config
export default defineConfig({
    site: `${BASE_URL}${BASE_PATH}`,
    base: BASE_PATH,
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
            title: "Slint Docs",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
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
                                items: [
                                    "guide/tooling/vscode",
                                    "guide/tooling/figma-inspector",
                                ],
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
                                                label: "Reactivity",
                                                slug: "guide/language/concepts/reactivity",
                                            },
                                            {
                                                label: "Reactivity vs React.js",
                                                slug: "guide/language/concepts/reactivity-vs-react",
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
                                    "guide/platforms/android",
                                    "guide/platforms/ios",
                                    "guide/platforms/web",
                                    "guide/platforms/other",
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
                        link: "reference/overview",
                        icon: "information",
                        items: [
                            {
                                label: "Overview",
                                slug: "reference/overview",
                            },
                            {
                                label: "Types and Properties",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Primitive Types",
                                        slug: "reference/primitive-types",
                                    },
                                    {
                                        label: "Common Properties & Callbacks",
                                        slug: "reference/common",
                                    },
                                    {
                                        label: "Colors & Brushes",
                                        slug: "reference/colors-and-brushes",
                                    },
                                    {
                                        label: "Timer",
                                        slug: "reference/timer",
                                    },
                                ],
                            },
                            {
                                label: "Visual Elements",
                                collapsed: true,
                                items: [
                                    {
                                        label: "Basic Elements",
                                        autogenerate: {
                                            directory: "reference/elements",
                                        },
                                    },
                                    {
                                        label: "Gestures",
                                        autogenerate: {
                                            directory: "reference/gestures",
                                        },
                                    },
                                    {
                                        label: "Keyboard Input",
                                        items: [
                                            {
                                                label: "Overview",
                                                slug: "reference/keyboard-input/overview",
                                            },
                                            {
                                                label: "FocusScope",
                                                slug: "reference/keyboard-input/focusscope",
                                            },
                                            {
                                                label: "TextInput",
                                                slug: "reference/keyboard-input/textinput",
                                            },
                                            {
                                                label: "TextInputInterface",
                                                slug: "reference/keyboard-input/textinputinterface",
                                            },
                                        ],
                                    },
                                    {
                                        label: "Basic Layouts",
                                        items: [
                                            {
                                                label: "Common Properties",
                                                slug: "reference/layouts/overview",
                                            },
                                            {
                                                label: "GridLayout",
                                                slug: "reference/layouts/gridlayout",
                                            },
                                            {
                                                label: "HorizontalLayout",
                                                slug: "reference/layouts/horizontallayout",
                                            },
                                            {
                                                label: "VerticalLayout",
                                                slug: "reference/layouts/verticallayout",
                                            },
                                        ],
                                    },
                                    {
                                        label: "Window",
                                        autogenerate: {
                                            directory: "reference/window",
                                        },
                                    },
                                ],
                            },
                            {
                                label: "Globals",
                                collapsed: true,
                                items: [
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
                                    {
                                        label: "Platform Namespace",
                                        slug: "reference/global-namespaces/platform",
                                    },
                                ],
                            },
                            {
                                label: "Std-Widgets",
                                collapsed: true,
                                items: [
                                    "reference/std-widgets/overview",
                                    "reference/std-widgets/style",
                                    {
                                        label: "Basic Widgets",
                                        autogenerate: {
                                            directory:
                                                "reference/std-widgets/basic-widgets",
                                        },
                                    },
                                    {
                                        label: "Views",
                                        autogenerate: {
                                            directory:
                                                "reference/std-widgets/views",
                                        },
                                    },
                                    {
                                        label: "Widget Layouts",
                                        autogenerate: {
                                            directory:
                                                "reference/std-widgets/layouts",
                                        },
                                    },
                                    {
                                        label: "Misc",
                                        autogenerate: {
                                            directory:
                                                "reference/std-widgets/misc",
                                        },
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
                                link: `${CPP_BASE_URL}`,
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "Rust ↗",
                                link: `${RUST_SLINT_CRATE_URL}`,
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "TypeScript ↗",
                                badge: {
                                    text: "beta",
                                    variant: "caution",
                                },
                                link: `${NODEJS_BASE_URL}`,
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "Python ↗",
                                badge: {
                                    text: "beta",
                                    variant: "caution",
                                },
                                link: `${PYTHON_BASE_URL}`,
                                attrs: { target: "_blank" },
                            },
                        ],
                    },
                ]),
                starlightLinksValidator({
                    errorOnLocalLinks: false,
                }),
            ],
            social: [
                {
                    icon: "github",
                    label: "GitHub",
                    href: "https://github.com/slint-ui/slint",
                },
                { icon: "x.com", label: "X", href: "https://x.com/slint_ui" },
                {
                    icon: "linkedin",
                    label: "Linkedin",
                    href: "https://www.linkedin.com/company/slint-ui",
                },
                {
                    icon: "mastodon",
                    label: "Mastodon",
                    href: "https://fosstodon.org/@slint",
                },
            ],
            favicon: "favicon.svg",
        }),
    ],
});
