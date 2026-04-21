// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const DIST = "dist";

async function* walk(dir: string): AsyncGenerator<string> {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
        const p = join(dir, entry.name);
        if (entry.isDirectory()) {
            yield* walk(p);
        } else if (entry.isFile() && p.endsWith(".md")) {
            yield p;
        }
    }
}

// Any in-prose component the markdown endpoint hasn't been taught to resolve
// will leak into the output as a literal tag, leaving the agent with no link
// to follow. List the components we know about here so a regression fails
// loudly rather than silently shipping unresolved tags.
const FORBIDDEN = [/<Link\b/];

test("no MDX components leak into the .md output", async () => {
    const offenders: { file: string; pattern: string }[] = [];
    for await (const file of walk(DIST)) {
        const body = await readFile(file, "utf8");
        for (const re of FORBIDDEN) {
            if (re.test(body)) {
                offenders.push({ file, pattern: re.source });
            }
        }
    }
    expect(offenders, "files contain unresolved MDX components").toEqual([]);
});
