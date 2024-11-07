// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import starlightLinksValidator from "starlight-links-validator";
import rehypeMermaid from "rehype-mermaid";
import addMermaidClass from "./src/utils/add-mermaid-classnames";

// https://astro.build/config
export default defineConfig({
    site: "https://snapshots.slint.dev/tng/",
    base: "/tng",
    markdown: {
        rehypePlugins: [addMermaidClass, rehypeMermaid],
    },
    integrations: [
        starlight({
            title: "",
            logo: {
                light: "./src/assets/slint-logo-simple-light.webp",
                dark: "./src/assets/slint-logo-simple-dark.webp",
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
            },
            sidebar: [
                {
                    label: "Getting started",
                    collapsed: true,
                    items: [
                        { label: "Welcome", slug: "getting-started/intro" },
                        {
                            label: "Which language?",
                            slug: "getting-started/which_language",
                        },
                    ],
                },
                {
                    label: "Guide",
                    collapsed: true,
                    items: [
                        { label: "Introduction", slug: "guide/intro" },
                        { label: "Live Preview", slug: "guide/preview" },
                        { label: "Basics", slug: "guide/basics" },
                        { label: "Types", slug: "guide/types" },
                    ],
                },
                {
                    label: "Elements Reference",
                    collapsed: true,
                    items: [
                        { label: "Overview", slug: "guide/intro" },
                        { label: "Live Preview", slug: "guide/preview" },
                        { label: "Basics", slug: "guide/basics" },
                        { label: "Types", slug: "guide/types" },
                    ],
                },
                {
                    label: "Std-Widgets",
                    collapsed: true,
                    items: [
                        "std-widgets/overview",
                        "std-widgets/aboutslint",
                        "std-widgets/button",
                        "std-widgets/checkbox",
                        "std-widgets/combobox",
                        "std-widgets/datepicker",
                        "std-widgets/gridbox",
                        "std-widgets/groupbox",
                        "std-widgets/horizontalbox",
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
                        "std-widgets/verticalbox",
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
                    label: "Native API",
                    collapsed: true,
                    items: [
                        {
                            label: "C++ ↗",
                            link: "https://docs.slint.dev/latest/docs/cpp/",
                        },
                        {
                            label: "Python ↗",
                            badge: { text: "beta", variant: "caution" },
                            link: "https://pypi.org/project/slint/",
                        },
                        {
                            label: "Rust ↗",
                            link: "https://docs.slint.dev/latest/docs/rust/slint/",
                        },
                        {
                            label: "TypeScript ↗",
                            link: "https://docs.slint.dev/latest/docs/node/",
                        },
                    ],
                },
            ],
        }),
    ],
});
