// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Runnable with: node --experimental-strip-types tests/markdown-endpoint.test.ts
// Uses the built-in node:test runner so no extra dependency is required.

import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
    markdownStaticPaths,
    renderMarkdownResponse,
    type MarkdownDocEntry,
} from "../src/utils/markdown-endpoint.ts";

function entry(over: Partial<MarkdownDocEntry>): MarkdownDocEntry {
    return { id: "", data: {}, body: "", ...over };
}

// A fake project root holding code files that `?raw` imports resolve to.
function projectRootWith(files: Record<string, string>): string {
    const root = mkdtempSync(join(tmpdir(), "md-endpoint-test-"));
    for (const [path, content] of Object.entries(files)) {
        const abs = join(root, path);
        mkdirSync(join(abs, ".."), { recursive: true });
        writeFileSync(abs, content);
    }
    return root;
}

test("root index (empty id) maps to the index.md slug", () => {
    const paths = markdownStaticPaths([
        entry({ id: "" }),
        entry({ id: "guide/intro" }),
    ]);
    assert.deepEqual(
        paths.map((p) => p.params.slug),
        ["index.md", "guide/intro.md"],
    );
    // The entry travels through as a prop for the GET handler.
    assert.equal(paths[1].props.entry.id, "guide/intro");
});

test("doc source extensions are stripped from route slugs", () => {
    const mdEntry = entry({ id: "guide/backend_linuxkms.md" });
    const paths = markdownStaticPaths([
        mdEntry,
        entry({ id: "guide/intro.mdx" }),
        entry({ id: "guide/v1.2/intro" }),
        entry({ id: "guide/file.name.markdown" }),
    ]);

    assert.deepEqual(
        paths.map((p) => p.params.slug),
        [
            "guide/backend_linuxkms.md",
            "guide/intro.md",
            "guide/v1.2/intro.md",
            "guide/file.name.md",
        ],
    );
    assert.equal(paths[0].props.entry, mdEntry);
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
                    href: "reference/language/expressions/#anchor",
                },
            },
        },
    );
    const text = await res.text();
    assert.match(
        text,
        /\[Expr\]\(\/docs\/reference\/language\/expressions\.md#anchor\)/,
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

test("a ?raw import feeding <Code> becomes a fenced block", async () => {
    const projectRoot = projectRootWith({
        "src/content/code/hello.sh": "echo hello\n",
    });
    const res = renderMarkdownResponse(
        entry({
            body: [
                "import script from '/src/content/code/hello.sh?raw'",
                "",
                '<Code code={script} lang="bash" />',
            ].join("\n"),
        }),
        { projectRoot },
    );
    const text = await res.text();
    assert.match(text, /```bash\necho hello\n```/);
    // The consumed import line is gone.
    assert.doesNotMatch(text, /import script/);
});

test("extractLines() slices a 1-based inclusive range and title is kept", async () => {
    const projectRoot = projectRootWith({
        "src/content/code/app.slint": "l1\nl2\nl3\nl4\nl5\n",
    });
    const res = renderMarkdownResponse(
        entry({
            body: [
                'import appWindow from "/src/content/code/app.slint?raw"',
                '<Code code={extractLines(appWindow, 2 ,4)} lang="slint" title="app.slint" />',
            ].join("\n"),
        }),
        { projectRoot },
    );
    const text = await res.text();
    assert.match(text, /```slint title="app\.slint"\nl2\nl3\nl4\n```/);
});

test("relative ?raw imports resolve against the page's directory", async () => {
    const projectRoot = projectRootWith({
        "scripts/build.sh": "make\n",
    });
    const res = renderMarkdownResponse(
        entry({
            filePath: "src/content/docs/guide/page.mdx",
            body: [
                "import script from './../../../../scripts/build.sh?raw'",
                '<Code code={script} lang="bash" />',
            ].join("\n"),
        }),
        { projectRoot },
    );
    const text = await res.text();
    assert.match(text, /```bash\nmake\n```/);
});

test("an unreadable ?raw import fails the build", () => {
    const projectRoot = projectRootWith({});
    const body = [
        "import gone from '/src/content/code/missing.cpp?raw'",
        '<Code code={gone} lang="cpp" />',
    ].join("\n");
    assert.throws(
        () => renderMarkdownResponse(entry({ body }), { projectRoot }),
        /cannot read "\/src\/content\/code\/missing\.cpp\?raw"/,
    );
});

test("a <Code> expression that is not a ?raw import fails the build", () => {
    const projectRoot = projectRootWith({});
    const body = '<Code code={someNewHelper(x)} lang="cpp" />';
    assert.throws(
        () => renderMarkdownResponse(entry({ body }), { projectRoot }),
        /does not reference a \?raw import/,
    );
});

test("a ?raw import that no recognized <Code> consumes fails the build", () => {
    const projectRoot = projectRootWith({
        "src/content/code/hello.sh": "echo hello\n",
    });
    const body = "import script from '/src/content/code/hello.sh?raw'";
    assert.throws(
        () => renderMarkdownResponse(entry({ body }), { projectRoot }),
        /feeds no <Code/,
    );
});

test("without projectRoot, code imports are left as-is", async () => {
    const body = "import script from '/src/content/code/x.sh?raw'";
    const res = renderMarkdownResponse(entry({ body }));
    const text = await res.text();
    assert.match(text, /import script from/);
});

test("API-reference signature HTML collapses to a ```cpp fence", async () => {
    const body = [
        '### <a id="title"></a> `title` <small>(virtual)</small>',
        "",
        '<pre class="shiki api-signature" style="--x:1"><code><span class="line">' +
            '<a href="../sharedstring/" class="api-link">SharedString</a>' +
            "<span> slint</span><span>::</span><span>title</span>" +
            "<span>() </span><span>const</span></span></code></pre>",
    ].join("\n");
    const text = await renderMarkdownResponse(entry({ body }), {
        apiSignatures: true,
    }).text();
    // The signature becomes a plain fence with the entities/tags stripped.
    assert.match(text, /```cpp\nSharedString slint::title\(\) const\n```/);
    // The empty deep-link anchor and the <small> marker are gone, the heading
    // text (kept as inline code) and the marker text remain.
    assert.doesNotMatch(text, /<a id=|<small>|<pre/);
    assert.match(text, /### `title` \(virtual\)/);
});

test("angle-bracket types in a signature are decoded, not left as entities", async () => {
    // Shiki escapes `<` as the hex reference `&#x3C;` (not `&lt;`); `>` is left
    // bare. Both the hex and named forms must decode.
    const body =
        '<pre class="api-signature"><code>' +
        "std::optional&#x3C; SharedPixelBuffer&lt;Rgba8Pixel> > take_snapshot()" +
        "</code></pre>";
    const text = await renderMarkdownResponse(entry({ body }), {
        apiSignatures: true,
    }).text();
    assert.match(
        text,
        /```cpp\nstd::optional< SharedPixelBuffer<Rgba8Pixel> > take_snapshot\(\)\n```/,
    );
    assert.doesNotMatch(text, /&#x|&lt;|&gt;/);
});
