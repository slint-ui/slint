// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore refid

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
        [
            "api/classes/slint-color",
            "api/classes/slint-sharedstring",
            "api/namespaces/slint",
        ].sort(),
    );
});

test("class page has frontmatter, include and brief", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(md, /^---\n/);
    assert.match(md, /title: "slint::Color Class"/);
    assert.match(md, /#include <slint.h>/);
    assert.match(md, /A `Color` represents an RGBA color\./);
});

test("a class defined in a private/ header advertises <slint.h>", () => {
    // slint::SharedString's `<includes>` points (via refid) at a file compound
    // whose location is under private/; that internal header must be rewritten
    // to the public umbrella header rather than shown verbatim.
    const md = convert().get("api/classes/slint-sharedstring")?.markdown ?? "";
    assert.match(md, /#include <slint.h>/);
    assert.doesNotMatch(md, /slint_string\.h/);
});

test("function signature, params, returns and note render", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    // Signature is an HTML <pre><code> block (so types can be linked); the text
    // (qualified name, return type, params) reads as before.
    assert.match(
        md,
        /<pre class="api-signature"><code>static Color slint::Color::from_argb_encoded\(uint32_t argb_encoded\)<\/code><\/pre>/,
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

test("namespace pages list their inner classes/structs with links", () => {
    const ns = convert().get("api/namespaces/slint")?.markdown ?? "";
    assert.match(ns, /## Classes/);
    // The leaf name is shown (prefix stripped) and linked to the class page.
    assert.match(ns, /- \[Color\]\(\.\.\/\.\.\/classes\/slint-color\/\)/);
});

test("documentation sections (sect1) render as headings with code blocks", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.match(md, /### Example/);
    // The code listing inside the section is a fenced cpp block, not inline prose.
    assert.match(
        md,
        /```cpp\nauto c = Color::from_argb_encoded\(0xff112233\);\n```/,
    );
});

test("signature parameter types are linked, the qualified name is not", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    // `void slint::Color::blend(const Color &other)` — the Color *parameter*
    // links to the page; the `Color` inside the qualified name must stay plain
    // (no substring mis-linking of the method name).
    assert.match(
        md,
        /void slint::Color::blend\(const <a href="\.\/">Color<\/a> &amp;other\)/,
    );
});

test("internal members (private_api/cbindgen_private) are not documented", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    // A `friend void private_api::touch_color(...)` declaration must be filtered
    // out, leaving no Friends section (it was the only friend).
    assert.doesNotMatch(md, /private_api/);
    assert.doesNotMatch(md, /## Friends/);
});

test("unresolved/no-ref text degrades gracefully (no crash, no empty links)", () => {
    const md = convert().get("api/classes/slint-color")?.markdown ?? "";
    assert.doesNotMatch(md, /\]\(\)/);
});
