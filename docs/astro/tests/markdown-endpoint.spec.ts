// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("doc page has a markdown sibling", async ({ request }) => {
    const res = await request.get("guide/language/coding/properties.md");
    expect(res.status()).toBe(200);
    expect(res.headers()["content-type"]).toContain("text/markdown");

    const body = await res.text();
    expect(body).toMatch(/^---\ntitle: "Properties"/);
    expect(body).toContain("## Assigning bindings");
});

test("root index has a markdown sibling at index.md", async ({ request }) => {
    const res = await request.get("index.md");
    expect(res.status()).toBe(200);
    expect(res.headers()["content-type"]).toContain("text/markdown");

    const body = await res.text();
    expect(body).toMatch(/^---\ntitle: "Overview"/);
});

// properties.mdx contains: <Link type="Expressions" />
// After Link-component resolution, that should appear in the markdown body
// as a real markdown link pointing at the Expressions page's .md sibling,
// so an agent can chain markdown fetches without rewriting URLs.
test("Link components resolve to a followable markdown link", async ({
    request,
}) => {
    const page = await request.get("guide/language/coding/properties.md");
    expect(page.status()).toBe(200);
    const body = await page.text();

    const match = body.match(
        /\[Expressions\]\((?<url>[^)]*expressions-and-statements[^)]*)\)/,
    );
    expect(
        match,
        "expected a [Expressions](...) markdown link in the body — " +
            "the <Link/> component should have been resolved",
    ).not.toBeNull();

    const url = match!.groups!.url;
    expect(url).toMatch(/\.md$/);

    const target = await request.get(url);
    expect(target.status()).toBe(200);
    expect(target.headers()["content-type"]).toContain("text/markdown");
});
