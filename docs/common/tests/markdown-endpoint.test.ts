// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Runnable with: node --experimental-strip-types tests/markdown-endpoint.test.ts
// Uses the built-in node:test runner so no extra dependency is required.

import assert from "node:assert/strict";
import { test } from "node:test";
import {
    markdownStaticPaths,
    renderMarkdownResponse,
    type MarkdownDocEntry,
} from "../src/utils/markdown-endpoint.ts";

function entry(over: Partial<MarkdownDocEntry>): MarkdownDocEntry {
    return { id: "", data: {}, body: "", ...over };
}

test("root index (empty id) maps to the index.md slug", () => {
    const paths = markdownStaticPaths([
        entry({ id: "" }),
        entry({ id: "guide/intro" }),
    ]);
    assert.deepEqual(
        paths.map((p) => p.params.slug),
        ["index", "guide/intro"],
    );
    // The entry travels through as a prop for the GET handler.
    assert.equal(paths[1].props.entry.id, "guide/intro");
});

test("renders YAML frontmatter and serves text/markdown", async () => {
    const res = renderMarkdownResponse(
        entry({
            data: { title: "Properties", description: "About properties" },
            body: "## Body\n",
        }),
    );
    assert.equal(
        res.headers.get("Content-Type"),
        "text/markdown; charset=utf-8",
    );
    assert.equal(res.headers.get("Vary"), "Accept");

    const text = await res.text();
    assert.match(
        text,
        /^---\ntitle: "Properties"\ndescription: "About properties"\n---\n/,
    );
    assert.match(text, /## Body/);
});

test("frontmatter omits absent fields and quotes safely", async () => {
    const res = renderMarkdownResponse(
        entry({ data: { title: 'He said "hi"' }, body: "" }),
    );
    const text = await res.text();
    // description line is absent, and the embedded quote is escaped.
    assert.match(text, /^---\ntitle: "He said \\"hi\\""\n---\n/);
    assert.doesNotMatch(text, /description:/);
});

test("without a linkMap, <Link> components are left untouched", async () => {
    const res = renderMarkdownResponse(
        entry({ body: 'before <Link type="Expressions" /> after' }),
    );
    const text = await res.text();
    assert.match(text, /before <Link type="Expressions" \/> after/);
});

test("with a linkMap, <Link> resolves to a .md sibling under the base path", async () => {
    const res = renderMarkdownResponse(
        entry({ body: '<Link type="Expressions" label="Expr" />' }),
        {
            basePath: "/docs/",
            linkMap: {
                Expressions: {
                    href: "guide/language/coding/expressions-and-statements/#anchor",
                },
            },
        },
    );
    const text = await res.text();
    assert.match(
        text,
        /\[Expr\]\(\/docs\/guide\/language\/coding\/expressions-and-statements\.md#anchor\)/,
    );
});

test("unknown link types are passed through unchanged", async () => {
    const res = renderMarkdownResponse(
        entry({ body: '<Link type="Nope" />' }),
        { basePath: "/docs/", linkMap: {} },
    );
    const text = await res.text();
    assert.match(text, /<Link type="Nope" \/>/);
});
