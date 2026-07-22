// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Serves a plain-markdown sibling for every doc page so AI agents can fetch
// docs without paying the HTML/JS overhead. Example:
//   …/docs/safety/reference/   -> rendered HTML page
//   …/docs/safety/reference.md -> this endpoint, raw markdown
//
// Mirrors the other docs sites' endpoints; see
// docs/common/src/utils/markdown-endpoint.ts. The safety manual uses the
// in-prose <Link> component (but no `?raw` code imports), so it passes the
// linkMap + its own base path and omits projectRoot.

import type { APIRoute, GetStaticPaths } from "astro";
import { getCollection, type CollectionEntry } from "astro:content";
import {
    markdownStaticPaths,
    renderMarkdownResponse,
} from "@slint/common-files/src/utils/markdown-endpoint";
import { linkMap } from "@slint/common-files/src/utils/utils";
import { SAFETY_DOCS_BASE_PATH } from "../safety-site-config.mjs";

export const getStaticPaths: GetStaticPaths = async () => {
    return markdownStaticPaths(await getCollection("docs"));
};

type Props = { entry: CollectionEntry<"docs"> };

export const GET: APIRoute<Props> = ({ props }) =>
    renderMarkdownResponse(props.entry, {
        basePath: SAFETY_DOCS_BASE_PATH,
        linkMap,
    });
