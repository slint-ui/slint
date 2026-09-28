// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Serves a plain-markdown sibling for every doc page so AI agents can fetch
// docs without paying the HTML/JS overhead. Example:
//   …/docs/cpp/api/slint/color/      -> rendered HTML page
//   …/docs/cpp/api/slint/color.md    -> this endpoint, raw markdown

import type { APIRoute, GetStaticPaths } from "astro";
import { getCollection, type CollectionEntry } from "astro:content";
import {
    markdownStaticPaths,
    renderMarkdownResponse,
} from "@slint/common-files/src/utils/markdown-endpoint";

export const getStaticPaths: GetStaticPaths = async () => {
    return markdownStaticPaths(await getCollection("docs"));
};

type Props = { entry: CollectionEntry<"docs"> };

export const GET: APIRoute<Props> = ({ props }) =>
    // The generated C++ pages embed `api-signature` HTML to fold into fences.
    renderMarkdownResponse(props.entry, { apiSignatures: true });
