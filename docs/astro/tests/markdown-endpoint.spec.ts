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
