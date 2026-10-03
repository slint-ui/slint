// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { renderLlmsTxt } from "@slint/common-files/src/utils/markdown-endpoint";

export const GET: APIRoute = async ({ site }) =>
    renderLlmsTxt(await getCollection("docs"), {
        title: "Slint SC Safety Manual",
        summary:
            "The Safety Manual for Slint SC, the ISO 26262 compliant subset of Slint.",
        basePath: import.meta.env.BASE_URL,
        site,
    });
