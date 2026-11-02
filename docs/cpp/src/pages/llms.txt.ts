// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { renderLlmsTxt } from "@slint/common-files/src/utils/markdown-endpoint";

export const GET: APIRoute = async ({ site }) =>
    renderLlmsTxt(await getCollection("docs"), {
        title: "Slint C++ API",
        summary:
            "Use Slint from C++ — compile .slint designs ahead of time or load them at run-time, and drive native UI from your C++ application.",
        basePath: import.meta.env.BASE_URL,
        site,
    });
