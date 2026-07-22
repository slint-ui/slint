// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Sync the language-specification chapters from their canonical location in
// the main Slint docs (docs/astro/src/content/docs/reference/language/) into
// this site's src/content/docs/language/ directory, which is gitignored.
//
// The chapters use relative links so that they resolve in both sites. Links
// that point outside the specification differ per site and are rewritten via
// LINK_MAP below.

import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Links that leave the specification directory: canonical (docs/astro) form
// on the left, safety-manual form on the right.
const LINK_MAP = new Map([["](../overview/)", "](../reference/)"]]);

const here = dirname(fileURLToPath(import.meta.url));
const source = join(here, "../../astro/src/content/docs/reference/language");
const target = join(here, "../src/content/docs/language");

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });

for (const entry of readdirSync(source)) {
    if (!entry.endsWith(".md") && !entry.endsWith(".mdx")) {
        cpSync(join(source, entry), join(target, entry), { recursive: true });
        continue;
    }
    let content = readFileSync(join(source, entry), "utf-8");
    for (const [from, to] of LINK_MAP) {
        content = content.replaceAll(from, to);
    }
    writeFileSync(join(target, entry), content);
}

console.log(`Synced language specification from ${source}`);
