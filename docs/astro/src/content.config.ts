// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineCollection, z } from "astro:content";
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
        schema: docsSchema({
            extend: z.object({
                // Language-specification chapter that is not part of the
                // Slint SC subset: the safety manual doesn't include it, and
                // its `{#sls.…}` identifiers, if any, are dropped.
                notInSC: z.boolean().optional(),
            }),
        }),
    }),
};
