// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { renderLlmsTxt } from "@slint/common-files/src/utils/markdown-endpoint";

export const GET: APIRoute = async ({ site }) =>
    renderLlmsTxt(await getCollection("docs"), {
        title: "Slint Docs",
        summary:
            "The guide and language reference for Slint, a declarative GUI toolkit for desktop, embedded, mobile, and web.",
        basePath: import.meta.env.BASE_URL,
        site,
    });
