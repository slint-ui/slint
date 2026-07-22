// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore refid eventloopmode

// Runnable with: node --experimental-strip-types tests/convert.test.ts
// Uses the built-in node:test runner so no extra dependency is required.

import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
    DoxygenConverter,
    tightenAngles,
    tightenTemplateSpacing,
} from "../scripts/lib/doxygen.ts";
import type {
    ConvertResult,
    GeneratedPage,
    SidebarLink,
} from "../scripts/lib/doxygen.ts";

const here = dirname(fileURLToPath(import.meta.url));
const xmlDir = join(here, "fixtures", "xml");

function build(): ConvertResult {
    return new DoxygenConverter(xmlDir).convert();
}

function convert(): Map<string, GeneratedPage> {
    return new Map(build().pages.map((p) => [p.slug, p]));
}

test("emits one page per page-kind compound with the expected slugs", () => {
    const pages = convert();
    assert.deepEqual(
        [...pages.keys()].sort(),
        [
            "api/slint",
            "api/slint/color",
            "api/slint/sharedstring",
            // Free functions and enums each become their own page too.
            "api/slint/run_event_loop",
            "api/slint/eventloopmode",
        ].sort(),
    );
});

test("namespace free functions and enums get their own page", () => {
    const fn = convert().get("api/slint/run_event_loop")?.markdown ?? "";
    assert.match(fn, /title: "slint::run_event_loop Function"/);
    assert.match(fn, /Enters the main event loop\./);

    const en = convert().get("api/slint/eventloopmode")?.markdown ?? "";
    assert.match(en, /title: "slint::EventLoopMode Enum"/);
    assert.match(
        en,
        /\| `QuitOnLastWindowClosed` \| Quit when the last window/,
    );
});

test("class page has frontmatter, include and brief", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    assert.match(md, /^---\n/);
    assert.match(md, /title: "slint::Color Class"/);
    assert.match(md, /#include <slint.h>/);
    assert.match(md, /A `Color` represents an RGBA color\./);
});

test("a class defined in a private/ header advertises <slint.h>", () => {
    // slint::SharedString's `<includes>` points (via refid) at a file compound
    // whose location is under private/; that internal header must be rewritten
    // to the public umbrella header rather than shown verbatim.
    const md = convert().get("api/slint/sharedstring")?.markdown ?? "";
    assert.match(md, /#include <slint.h>/);
    assert.doesNotMatch(md, /slint_string\.h/);
});

test("every type page opens with a declaration line", () => {
    // Non-template type: a plain `<keyword> <name>;` declaration.
    const color = convert().get("api/slint/color")?.markdown ?? "";
    assert.match(color, /```cpp\nclass Color;\n```/);

    // Template type: the parameters precede the keyword + name, so it is not a
    // bare `template <typename T>` but a real declaration.
    const shared = convert().get("api/slint/sharedstring")?.markdown ?? "";
    assert.match(
        shared,
        /```cpp\ntemplate <typename T>\nclass SharedString;\n```/,
    );
});

test("function signature, params, returns and note render", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
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
    const md = convert().get("api/slint/color")?.markdown ?? "";
    assert.match(md, /\| Value \| Description \|/);
    assert.match(md, /\| `Argb` \| 32-bit ARGB\. \|/);
    assert.match(md, /enum class Format/);
});

test("code blocks and lists render", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    assert.match(md, /```cpp\nColor c;\nauto r = c\.red\(\);\n```/);
    assert.match(md, /- 0 means no red\./);
});

test("cross-references resolve to relative links with anchors", () => {
    const ns = convert().get("api/slint")?.markdown ?? "";
    // compound ref -> page link, relative so it resolves under any deploy base
    // (from api/slint to api/slint/color).
    assert.match(ns, /\[Color\]\(color\/\)/);
    const cls = convert().get("api/slint/color")?.markdown ?? "";
    // member anchor is stable and emitted as an HTML id
    assert.match(cls, /<a id="from_argb_encoded"><\/a>/);
    assert.match(cls, /\[the website\]\(https:\/\/slint.dev\)/);
});

test("namespace pages list their types (classes and structs together) with links", () => {
    const ns = convert().get("api/slint")?.markdown ?? "";
    // Classes and structs are listed together under a kind-neutral heading.
    assert.match(ns, /## Types/);
    // The leaf name is shown (prefix stripped) and linked to the type page.
    assert.match(ns, /- \[Color\]\(color\/\)/);
});

test("namespace pages link to function/enum pages instead of inlining them", () => {
    const ns = convert().get("api/slint")?.markdown ?? "";
    // Free functions and enums are listed as links to their own pages…
    assert.match(ns, /## Functions\n- \[run_event_loop\]\(run_event_loop\/\)/);
    assert.match(ns, /## Enumerations\n- \[EventLoopMode\]\(eventloopmode\/\)/);
    // …not rendered inline as member sections on the namespace page.
    assert.doesNotMatch(ns, /### <a id="run_event_loop">/);
});

test("overridable virtual functions are flagged in the heading, not the signature", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    // virtual, not final -> (virtual) marker as a smaller-font suffix (name first).
    assert.match(
        md,
        /### <a id="on_changed"><\/a> `on_changed` <small>\(virtual\)<\/small>\n/,
    );
    // pure-virtual (=0) -> (pure virtual) suffix.
    assert.match(
        md,
        /### <a id="on_required"><\/a> `on_required` <small>\(pure virtual\)<\/small>\n/,
    );
    // virtual but `final` -> no marker (cannot be overridden further).
    assert.match(md, /### <a id="on_sealed"><\/a> `on_sealed`\n/);
    // a plain non-virtual member is never flagged.
    assert.match(md, /### <a id="red"><\/a> `red`\n/);
    // `virtual` is dropped from the signature itself (the marker conveys it).
    assert.match(
        md,
        /<pre class="api-signature"><code>void slint::Color::on_changed\(\)<\/code><\/pre>/,
    );
});

test("documentation sections (sect1) render as headings with code blocks", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    assert.match(md, /### Example/);
    // The code listing inside the section is a fenced cpp block, not inline prose.
    assert.match(
        md,
        /```cpp\nauto c = Color::from_argb_encoded\(0xff112233\);\n```/,
    );
});

test("signature parameter types are linked, the qualified name is not", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    // `void slint::Color::blend(const Color &other)` — the Color *parameter*
    // links to the page; the `Color` inside the qualified name must stay plain
    // (no substring mis-linking of the method name).
    assert.match(
        md,
        /void slint::Color::blend\(const <a href="\.\/">Color<\/a> &amp;other\)/,
    );
});

test("internal members (private_api/cbindgen_private) are not documented", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    // A `friend void private_api::touch_color(...)` declaration must be filtered
    // out, leaving no Friends section (it was the only friend).
    assert.doesNotMatch(md, /private_api/);
    assert.doesNotMatch(md, /## Friends/);
});

test("unresolved/no-ref text degrades gracefully (no crash, no empty links)", () => {
    const md = convert().get("api/slint/color")?.markdown ?? "";
    assert.doesNotMatch(md, /\]\(\)/);
});

test("sidebar hoists the implicit root namespace directly under API Reference", () => {
    const { sidebar } = build();
    const items = sidebar as SidebarLink[];

    // The `slint` root is implicit: its "Overview" sits at the top level (no
    // intermediate "slint" group), linking to the namespace page by slug.
    assert.deepEqual(items[0], { label: "Overview", slug: "api/slint" });

    // Classes and structs are listed together as page links by leaf name.
    const color = items.find((i) => i.label === "Color");
    const shared = items.find((i) => i.label === "SharedString");
    assert.equal(color?.slug, "api/slint/color");
    assert.equal(shared?.slug, "api/slint/sharedstring");

    // Free functions and enums link to their own page (not an anchor).
    const fn = items.find((i) => i.label === "run_event_loop");
    const en = items.find((i) => i.label === "EventLoopMode");
    assert.equal(fn?.slug, "api/slint/run_event_loop");
    assert.equal(en?.slug, "api/slint/eventloopmode");
});

test("tightenAngles collapses bracket padding but keeps comma/keyword spacing", () => {
    assert.equal(
        tightenAngles("std::optional< SharedPixelBuffer< Rgba8Pixel > >"),
        "std::optional<SharedPixelBuffer<Rgba8Pixel>>",
    );
    assert.equal(
        tightenAngles("slint::Model< ModelData >"),
        "slint::Model<ModelData>",
    );
    // Spaces after a comma between arguments stay.
    assert.equal(tightenAngles("map< K, V >"), "map<K, V>");
    // The `template <…>` keyword spacing (space before `<`) is left alone.
    assert.equal(
        tightenAngles("template <typename T>"),
        "template <typename T>",
    );
});

test("tightenTemplateSpacing keeps cross-reference links aligned", () => {
    const text = "optional< Foo< Bar > > f()";
    const foo = text.indexOf("Foo");
    const bar = text.indexOf("Bar");
    const { text: out, links } = tightenTemplateSpacing(text, [
        { start: foo, end: foo + 3, url: "/foo/" },
        { start: bar, end: bar + 3, url: "/bar/" },
    ]);
    assert.equal(out, "optional<Foo<Bar>> f()");
    // The remapped offsets still bound exactly the linked type names.
    assert.equal(out.slice(links[0].start, links[0].end), "Foo");
    assert.equal(out.slice(links[1].start, links[1].end), "Bar");
    assert.deepEqual(
        links.map((l) => l.url),
        ["/foo/", "/bar/"],
    );
});
