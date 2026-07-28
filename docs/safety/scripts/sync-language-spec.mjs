// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Sync the language-specification chapters from their canonical location in
// the main Slint docs (docs/astro/src/content/docs/reference/language/) into
// this site's src/content/docs/language/ directory, which is gitignored.
// Chapters with `notInSC: true` in their frontmatter cover the full language
// only and are left out.
//
// The chapters use relative links so that they resolve in both sites. Links
// that point outside the specification differ per site and are rewritten via
// LINK_MAP below.

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Links that leave the specification directory: canonical (docs/astro) form
// on the left, safety-manual form on the right.
const LINK_MAP = new Map([["](../overview/)", "](../reference/)"]]);

function isNotInSC(content) {
    const frontmatter = content.match(/^---\r?\n([\s\S]*?)\r?\n---/);
    return frontmatter != null && /^notInSC:\s*true\s*$/m.test(frontmatter[1]);
}

const here = dirname(fileURLToPath(import.meta.url));
const source = join(here, "../../astro/src/content/docs/reference/language");
const target = join(here, "../src/content/docs/language");

mkdirSync(target, { recursive: true });

// Write only files whose content changed and remove stale ones, so that a
// running `astro dev` watcher sees the minimal set of file events instead of
// the whole directory disappearing and reappearing.
const wanted = new Set();
for (const entry of readdirSync(source)) {
    if (!entry.endsWith(".md") && !entry.endsWith(".mdx")) {
        continue;
    }
    let content = readFileSync(join(source, entry), "utf-8");
    if (isNotInSC(content)) {
        continue;
    }
    wanted.add(entry);
    for (const [from, to] of LINK_MAP) {
        content = content.replaceAll(from, to);
    }
    const targetFile = join(target, entry);
    if (!existsSync(targetFile) || readFileSync(targetFile, "utf-8") !== content) {
        writeFileSync(targetFile, content);
    }
}
for (const entry of readdirSync(target)) {
    if (!wanted.has(entry)) {
        rmSync(join(target, entry), { recursive: true });
    }
}

console.log(`Synced language specification from ${source}`);
