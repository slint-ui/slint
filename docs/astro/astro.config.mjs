// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// @ts-check
import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";
import starlight from "@astrojs/starlight";
import starlightSidebarTopics from "starlight-sidebar-topics";
import { slintStarlightFaviconHead } from "@slint/common-files/src/utils/starlight-favicon-head";
import {
    SLINT_STARLIGHT_TRAILING_SLASH,
    slintStarlightLinksValidatorPlugin,
    slintStarlightMarkdownRehypeExternalLinksOnly,
} from "@slint/common-files/src/utils/starlight-site-defaults";
import { slintStarlightSocial } from "@slint/common-files/src/utils/starlight-social";
import {
    BASE_PATH,
    BASE_URL,
    CPP_BASE_URL,
    RUST_SLINT_CRATE_URL,
    NODEJS_BASE_URL,
    PYTHON_BASE_URL,
} from "@slint/common-files/src/utils/site-config";

const experimentalDocs = process.env.SLINT_ENABLE_EXPERIMENTAL_FEATURES === "1";

// Starlight prepends the base path to every sidebar link that is not a full
// URL (http/https). Strip BASE_PATH so the re-added prefix produces the
// intended absolute path (e.g. "/docs/../cpp/" -> "../cpp/" -> Starlight
// adds base -> "/docs/../cpp/" which the browser resolves to "/cpp/").
const sidebarHref = (/** @type {string} */ url) =>
    url.startsWith(BASE_PATH) ? url.slice(BASE_PATH.length) : url;

// https://astro.build/config
export default defineConfig({
    site: `${BASE_URL}${BASE_PATH}`,
    base: BASE_PATH,
    trailingSlash: SLINT_STARLIGHT_TRAILING_SLASH,
    markdown: slintStarlightMarkdownRehypeExternalLinksOnly(),
    integrations: [
        sitemap(),
        starlight({
            title: "Slint Docs",
            logo: {
                src: "./src/assets/slint-logo-small-light.svg",
            },
            customCss: [
                "@slint/common-files/src/styles/starlight-slint-custom.css",
                "@slint/common-files/src/styles/starlight-slint-theme.css",
            ],

            components: {
                Footer: "@slint/common-files/src/components/Footer.astro",
                Header: "@slint/common-files/src/components/HeaderSlintDocs.astro",
                Banner: "@slint/common-files/src/components/Banner.astro",
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
                                    {
                                        label: "Other Editors",
                                        collapsed: true,
                                        items: [
                                            "guide/tooling/manual-setup",
                                            "guide/tooling/kate",
                                            "guide/tooling/qt-creator",
                                            "guide/tooling/helix",
                                            "guide/tooling/neo-vim",
                                            "guide/tooling/sublime-text",
                                            "guide/tooling/jetbrains-ide",
                                            "guide/tooling/zed",
                                        ],
                                    },
                                    "guide/tooling/live-preview",
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
                                    "guide/development/best-practices",
                                    "guide/development/third-party-libraries",
                                ],
                            },
                            {
                                label: "Platforms",
                                collapsed: true,
                                items: [
                                    "guide/platforms/desktop",
                                    "guide/platforms/embedded",
                                    {
                                        label: "Mobile",
                                        collapsed: true,
                                        items: [
                                            "guide/platforms/mobile/general",
                                            "guide/platforms/mobile/android",
                                            "guide/platforms/mobile/ios",
                                        ],
                                    },
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
                            ...(experimentalDocs
                                ? [
                                      {
                                          label: "Experimental Features",
                                          collapsed: true,
                                          items: [
                                              {
                                                  label: "Overview",
                                                  slug: "guide/experimental/overview",
                                              },
                                              {
                                                  label: "AI Coding Assistants",
                                                  slug: "guide/experimental/ai-coding-assistants",
                                              },
                                              {
                                                  label: "FlexboxLayout",
                                                  slug: "guide/experimental/flexboxlayout",
                                              },
                                              {
                                                  label: "Drag and Drop",
                                                  slug: "guide/experimental/drag-and-drop",
                                              },
                                              {
                                                  label: "Interface",
                                                  slug: "guide/experimental/interface",
                                              },
                                              {
                                                  label: "ComponentContainer",
                                                  slug: "guide/experimental/component-container",
                                              },
                                              {
                                                  label: "Window.hide()",
                                                  slug: "guide/experimental/window-hide",
                                              },
                                              {
                                                  label: "Library Modules",
                                                  slug: "guide/experimental/library-modules",
                                              },
                                          ],
                                      },
                                  ]
                                : []),
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
                                            // FlexboxLayout is experimental. When it ships, drop
                                            // `draft: true` from flexboxlayout.mdx and uncomment:
                                            // {
                                            //     label: "FlexboxLayout",
                                            //     slug: "reference/layouts/flexboxlayout",
                                            // },
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
                                    {
                                        label: "FontWeight Namespace",
                                        slug: "reference/global-namespaces/font-weight",
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
                                        label: "Globals",
                                        autogenerate: {
                                            directory:
                                                "reference/std-widgets/globals",
                                        },
                                    },
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
                                link: sidebarHref(CPP_BASE_URL),
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "Rust ↗",
                                link: sidebarHref(RUST_SLINT_CRATE_URL),
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "TypeScript ↗",
                                badge: {
                                    text: "beta",
                                    variant: "caution",
                                },
                                link: sidebarHref(NODEJS_BASE_URL),
                                attrs: { target: "_blank" },
                            },
                            {
                                label: "Python ↗",
                                badge: {
                                    text: "beta",
                                    variant: "caution",
                                },
                                link: sidebarHref(PYTHON_BASE_URL),
                                attrs: { target: "_blank" },
                            },
                        ],
                    },
                ]),
                slintStarlightLinksValidatorPlugin(),
            ],
            social: slintStarlightSocial,
            favicon: "favicon.svg",
            head: slintStarlightFaviconHead(
                (filename) => `${BASE_PATH}/${filename}`,
            ),
        }),
    ],
});
