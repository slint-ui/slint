// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore eventloopmode

// Integration test: a generated page through the shared .md endpoint. Uses the
// real Shiki highlighter (so the HTML matches production) but asserts the fenced
// C++ text, not the HTML, so it survives Shiki output changes.
//
// Runnable with: node --experimental-strip-types tests/md-endpoint.test.ts

import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { createHighlighter } from "shiki";
import { renderMarkdownResponse } from "@slint/common-files/src/utils/markdown-endpoint.ts";
import { DoxygenConverter } from "../scripts/lib/doxygen.ts";

const xmlDir = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "xml");

/** Render a generated page as the `.md` endpoint serves it, via real Shiki. */
async function renderMd(slug: string): Promise<string> {
    const highlighter = await createHighlighter({
        themes: ["light-plus", "dark-plus"],
        langs: ["cpp"],
    });
    const page = new DoxygenConverter(xmlDir, { highlighter })
        .convert()
        .pages.find((p) => p.slug === slug);
    assert.ok(page, `expected a generated page for ${slug}`);
    return renderMarkdownResponse(
        { id: slug, data: {}, body: page.markdown },
        { apiSignatures: true },
    ).text();
}

test("a class's member signatures are served as fenced C++, not HTML", async () => {
    const md = await renderMd("api/slint/color");
    // The signature reads as the real C++ declaration inside a ```cpp fence …
    assert.match(
        md,
        /```cpp\nstatic Color slint::Color::from_argb_encoded\(uint32_t argb_encoded\)\n```/,
    );
    // … and a member with a linked parameter type keeps the type as plain text.
    assert.match(
        md,
        /```cpp\nvoid slint::Color::blend\(const Color &other\)\n```/,
    );
    // No raw signature HTML, Shiki wrappers or heading anchors leak through.
    assert.doesNotMatch(md, /<pre|class="[^"]*api-signature|<span|<a id=/);
});

test("member headings keep the member name without the anchor markup", async () => {
    const md = await renderMd("api/slint/color");
    assert.match(md, /### `from_argb_encoded`/);
    assert.doesNotMatch(md, /<a id="from_argb_encoded">/);
});
