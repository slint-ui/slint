// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

export const collections = {
    docs: defineCollection({
        loader: glob({
            base: "src/content/docs",
            pattern: "**/[^_]*.{md,mdx}",
        }),
        schema: docsSchema(),
    }),
};
