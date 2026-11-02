// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { renderLlmsTxt } from "@slint/common-files/src/utils/markdown-endpoint";

export const GET: APIRoute = async ({ site }) =>
    renderLlmsTxt(await getCollection("docs"), {
        title: "Slint for Node.js",
        summary:
            "Use Slint from Node.js, Deno, or Bun — install slint-ui, load .slint files, and run native UI windows.",
        basePath: import.meta.env.BASE_URL,
        site,
    });
