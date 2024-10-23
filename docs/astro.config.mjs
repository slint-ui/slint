// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import fs from "node:fs";

// https://astro.build/config
export default defineConfig({
    site: "https://snapshots.slint.dev/tng/",
    base: "/tng",
    integrations: [
        starlight({
            title: "Slint Language Docs",
            customCss: ["./src/styles/custom.css"],
            expressiveCode: {
                styleOverrides: { borderRadius: "0.2rem" },
                themes: ["dracula", "catppuccin-latte"],
                shiki: {
                    langs: [
                        JSON.parse(
                            fs.readFileSync(
                                "src/misc/Slint-tmLanguage.json",
                                "utf-8",
                            ),
                        ),
                    ],
                },
            },
            social: {
                github: "https://github.com/slint-ui/slint",
                "x.com": "https://x.com/slint_ui",
            },
            sidebar: [
                {
                    label: "Getting started",
                    items: [
                        { label: "Welcome", slug: "getting-started/intro" },
                    ],
                },
                {
                    label: "Guide",
                    items: [
                        // Each item here is one entry in the navigation menu.
                        { label: "Introduction", slug: "guide/philosophy" },
                        { label: "Types", slug: "guide/types" },
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
                    label: "Elements",
                    autogenerate: { directory: "elements" },
                },

                {
                    label: "Native API",
                    items: [
                        {
                            label: "C++",
                            link: "https://docs.slint.dev/latest/docs/cpp/",
                        },
                        {
                            label: "Python",
                            badge: { text: "beta", variant: "caution" },
                            link: "https://pypi.org/project/slint/",
                        },
                        {
                            label: "Rust",
                            link: "https://docs.slint.dev/latest/docs/rust/slint/",
                        },
                        {
                            label: "TypeScript",
                            link: "https://docs.slint.dev/latest/docs/node/",
                        },
                    ],
                },
            ],
        }),
    ],
});
