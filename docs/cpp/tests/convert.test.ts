// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Runnable with: node --experimental-strip-types tests/convert.test.ts
// Uses the built-in node:test runner so no extra dependency is required.

import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { DoxygenConverter } from "../scripts/lib/doxygen.ts";
import type { GeneratedPage } from "../scripts/lib/doxygen.ts";

const here = dirname(fileURLToPath(import.meta.url));
const xmlDir = join(here, "fixtures", "xml");

function convert(): Map<string, GeneratedPage> {
    const pages = new DoxygenConverter(xmlDir).convert();
    return new Map(pages.map((p) => [p.slug, p]));
}

test("emits one page per page-kind compound with the expected slugs", () => {
    const pages = convert();
    assert.deepEqual(
        [...pages.keys()].sort(),
        ["api/classes/slint-color", "api/namespaces/slint"].sort(),
    );
});

test("class page has frontmatter, include and brief", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(md, /^---\n/);
    assert.match(md, /title: "slint::Color Class"/);
    assert.match(md, /#include <slint.h>/);
    assert.match(md, /A `Color` represents an RGBA color\./);
});

test("function signature, params, returns and note render", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(
        md,
        /static Color slint::Color::from_argb_encoded\(uint32_t argb_encoded\)/,
    );
    assert.match(md, /\*\*Parameters:\*\*/);
    assert.match(md, /`argb_encoded` — The 32-bit encoded color\./);
    assert.match(md, /\*\*Returns:\*\*/);
    assert.match(md, /:::note/);
    assert.match(md, /\*\*not\*\*/);
});

test("enum renders as a value table", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(md, /\| Value \| Description \|/);
    assert.match(md, /\| `Argb` \| 32-bit ARGB\. \|/);
    assert.match(md, /enum class Format/);
});

test("code blocks and lists render", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(md, /```cpp\nColor c;\nauto r = c\.red\(\);\n```/);
    assert.match(md, /- 0 means no red\./);
});

test("cross-references resolve to relative links with anchors", () => {
    const ns = convert().get("api/namespaces/slint")?.markdown ?? "";
    // compound ref -> page link, relative so it resolves under any deploy base
    // (from api/namespaces/slint to api/classes/slint-color).
    assert.match(ns, /\[Color\]\(\.\.\/\.\.\/classes\/slint-color\/\)/);
    const cls = convert().get("api/classes/slint-color")?.markdown ?? "";
    // member anchor is stable and emitted as an HTML id
    assert.match(cls, /<a id="from_argb_encoded"><\/a>/);
    assert.match(cls, /\[the website\]\(https:\/\/slint.dev\)/);
});

test("unresolved/no-ref text degrades gracefully (no crash, no empty links)", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.doesNotMatch(md, /\]\(\)/);
});
