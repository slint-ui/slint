// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Serves a plain-markdown sibling for every doc page so AI agents can fetch
// docs without paying the HTML/JS overhead. Example:
//   /docs/reference/language/properties/      -> rendered HTML page
//   /docs/reference/language/properties.md    -> this endpoint, raw markdown

import { fileURLToPath } from "node:url";
import type { APIRoute, GetStaticPaths } from "astro";
import { root } from "astro:config/server";
import { getCollection, type CollectionEntry } from "astro:content";
import {
    markdownStaticPaths,
    renderMarkdownResponse,
} from "@slint/common-files/src/utils/markdown-endpoint";
import { linkMap } from "@slint/common-files/src/utils/utils";
import { BASE_PATH } from "@slint/common-files/src/utils/site-config";

export const getStaticPaths: GetStaticPaths = async () => {
    return markdownStaticPaths(await getCollection("docs"));
};

type Props = { entry: CollectionEntry<"docs"> };

// Project root for resolving `?raw` code imports.
const projectRoot = fileURLToPath(root);

export const GET: APIRoute<Props> = ({ props }) =>
    renderMarkdownResponse(props.entry, {
        basePath: BASE_PATH,
        linkMap,
        projectRoot,
    });
