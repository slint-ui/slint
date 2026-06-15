// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { test } from "node:test";
import assert from "node:assert/strict";
import { parse } from "node-html-parser";
import {
    classify,
    decode,
    firstSentence,
    preText,
    render,
} from "../scripts/rustdoc-to-llms.ts";

test("preText strips tags and decodes entities while keeping generics", () => {
    // rustdoc renders generics as &lt;…&gt; and wraps types in <a>/<span>. The
    // tags must be stripped before the entities are decoded, otherwise <Self>
    // would look like a tag and get removed.
    const pre = parse(
        '<pre class="rust item-decl"><code>pub fn f() -&gt; <a class="struct">Weak</a>&lt;Self&gt;</code></pre>',
    ).querySelector("pre");
    assert.ok(pre);
    assert.equal(preText(pre).trim(), "pub fn f() -> Weak<Self>");
});

test("render handles paragraphs, inline code, links and lists", () => {
    const div = parse(
        '<div class="docblock"><p>Use <code>Foo</code> see <a href="bar.html">bar</a>.</p><ul><li>one</li><li>two</li></ul></div>',
    ).querySelector("div");
    assert.ok(div);
    const md = render(div)
        .replace(/\n{3,}/g, "\n\n")
        .trim();
    assert.match(md, /Use `Foo` see \[bar\]\(bar\.html\)\./);
    assert.match(md, /- one\n- two/);
});

test("render keeps the text of in-page anchor links but drops the href", () => {
    const p = parse(
        '<p>see <a href="#renderers">Renderers</a></p>',
    ).querySelector("p");
    assert.ok(p);
    assert.equal(render(p).trim(), "see Renderers");
});

test("render emits method signatures (h4.code-header) as rust code blocks", () => {
    const h4 = parse(
        '<h4 class="code-header">pub fn show(&self) -&gt; Result&lt;(), PlatformError&gt;</h4>',
    ).querySelector("h4");
    assert.ok(h4);
    assert.equal(
        render(h4).trim(),
        "```rust\npub fn show(&self) -> Result<(), PlatformError>\n```",
    );
});

test("classify maps rustdoc file names to a path and kind", () => {
    assert.deepEqual(
        classify("slint", "slint/platform/enum.WindowEvent.html"),
        {
            path: "slint::platform::WindowEvent",
            kind: "Enum",
            isIndex: false,
        },
    );
    assert.deepEqual(classify("slint", "slint/struct.Window.html"), {
        path: "slint::Window",
        kind: "Struct",
        isIndex: false,
    });
    assert.deepEqual(classify("slint", "slint/index.html"), {
        path: "slint",
        kind: "Module",
        isIndex: true,
    });
    assert.equal(classify("slint", "slint/all.html"), null);
});

test("firstSentence stops at the first period", () => {
    assert.equal(
        firstSentence("A declarative GUI toolkit. More detail follows."),
        "A declarative GUI toolkit.",
    );
});

test("decode resolves named and numeric entities", () => {
    assert.equal(decode("a&lt;b&gt;c&amp;d&#39;e"), "a<b>c&d'e");
});
