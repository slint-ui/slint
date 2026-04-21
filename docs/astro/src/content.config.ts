// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

const experimentalDocs = process.env.SLINT_ENABLE_EXPERIMENTAL_FEATURES === "1";

const docsPattern = [
    "**/[^_]*.{md,mdx}",
    ...(experimentalDocs ? [] : ["!guide/experimental/**"]),
];

export const collections = {
    docs: defineCollection({
        loader: glob({ base: "src/content/docs", pattern: docsPattern }),
        schema: docsSchema(),
    }),
};
