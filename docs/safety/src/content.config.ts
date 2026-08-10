// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineCollection, z } from "astro:content";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

export const collections = {
    docs: defineCollection({
        loader: docsLoader(),
        schema: docsSchema({
            extend: z.object({
                // Navigational page, like a section landing page: it states
                // no requirements, so its paragraphs carry no `{#sls.…}`
                // identifiers. Defaults to true for every other page under
                // `reference/`, which the completeness check in
                // rehype-sls-ids.mjs holds to the corpus rules.
                normative: z.boolean().optional(),
                // Carried over by the synced specification and property-types
                // chapters: the manual includes the content they wrap in <SC>.
                SC: z.boolean().optional(),
            }),
        }),
    }),
};
