// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import type { APIRoute } from "astro";

export const GET: APIRoute = ({ site }) => {
    const sitemap = new URL(
        `${import.meta.env.BASE_URL}sitemap-index.xml`,
        site,
    );
    return new Response(`User-agent: *\nAllow: /\n\nSitemap: ${sitemap}\n`, {
        headers: { "Content-Type": "text/plain; charset=utf-8" },
    });
};
