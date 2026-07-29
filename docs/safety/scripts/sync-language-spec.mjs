// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Sync the language-specification chapters from their canonical location in
// the main Slint docs (docs/astro/src/content/docs/reference/language/) into
// this site's src/content/docs/language/ directory, which is gitignored.
// Only chapters that opt into the SC subset with `SC: true` in their
// frontmatter are brought over, and only the content they wrap in <SC>.
//
// The chapters use relative links so that they resolve in both sites. Links
// that point outside the specification differ per site and are rewritten via
// LINK_MAP below. The copies written here then get every link rewritten from
// the site base, because starlight-links-validator skips relative links
// entirely: a relative link would go unchecked and could rot into a dead one
// unnoticed.

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Links that leave the specification directory: canonical (docs/astro) form
// on the left, safety-manual form on the right.
const LINK_MAP = new Map([["](../overview/)", "](../reference/)"]]);

function isSC(content) {
    const frontmatter = content.match(/^---\r?\n([\s\S]*?)\r?\n---/);
    return frontmatter != null && /^SC:\s*true\s*$/m.test(frontmatter[1]);
}

// Keep only the certified content: the frontmatter, the imports, and the
// <SC>/<OnlyInSC> blocks (an <SC> block may contain a nested <NotInSC> that the
// manual omits at runtime). Everything outside a block is main-documentation
// only and is dropped, so nothing uncertified reaches the safety manual -- and
// with it only links this site can resolve (a dropped block outside the subset
// links to chapters the manual doesn't serve, which link validation rejects),
// and no throwing component in the dropped part (e.g. a CodeSnippetMD for a
// main-docs-only image) can break the build here.
//
// An <OnlyInSC> nests inside an <SC>, so count nesting depth rather than a
// single block, or the inner </OnlyInSC> would end the outer <SC> early.
function keepOnlySC(content) {
    const out = [];
    let delimiters = 0;
    let depth = 0;
    for (const line of content.split("\n")) {
        const t = line.trim();
        if (delimiters < 2) {
            out.push(line);
            if (t === "---") delimiters++;
            continue;
        }
        if (depth > 0) {
            out.push(line);
            if (t === "<SC>" || t === "<OnlyInSC>") depth++;
            else if (t === "</SC>" || t === "</OnlyInSC>") depth--;
            continue;
        }
        if (t === "<SC>" || t === "<OnlyInSC>") {
            out.push(line);
            depth++;
            continue;
        }
        // Outside a certified block keep only imports and blank lines (for
        // spacing); drop the main-docs-only content.
        if (t === "" || t.startsWith("import ") || t.startsWith("{/*")) {
            out.push(line);
        }
    }
    return out.join("\n");
}

// Rewrite the markdown links of a page served at `pageUrl` from the site root,
// so that starlight-links-validator checks them: it skips relative links
// rather than resolving them. Resolving against the page's own URL keeps the
// sources readable, since they stay relative. remark-base-links.mjs adds the
// base at build time.
function linksFromRoot(content, pageUrl) {
    return content.replace(/\]\((\.\.?\/[^)]*)\)/g, (_, url) => {
        const resolved = new URL(url, `https://slint.dev${pageUrl}`);
        return `](${resolved.pathname}${resolved.hash})`;
    });
}

// URL of a synced page below `sectionUrl`; an index page heads its section.
function pageUrl(sectionUrl, entry) {
    const stem = entry.replace(/\.mdx?$/, "");
    return stem === "index" ? sectionUrl : `${sectionUrl}${stem}/`;
}

// Copy the files under `sourceDir` accepted by `accept` into `targetDir`,
// optionally transforming each file, writing only changed files and removing
// stale ones (minimal watcher churn).
function syncDir(sourceDir, targetDir, accept, transform = (c) => c) {
    mkdirSync(targetDir, { recursive: true });
    const wanted = new Set();
    for (const entry of readdirSync(sourceDir)) {
        if (!entry.endsWith(".md") && !entry.endsWith(".mdx")) {
            continue;
        }
        const content = readFileSync(join(sourceDir, entry), "utf-8");
        if (!accept(content)) {
            continue;
        }
        wanted.add(entry);
        const out = transform(content, entry);
        const targetFile = join(targetDir, entry);
        if (!existsSync(targetFile) || readFileSync(targetFile, "utf-8") !== out) {
            writeFileSync(targetFile, out);
        }
    }
    for (const entry of readdirSync(targetDir)) {
        if (!wanted.has(entry)) {
            rmSync(join(targetDir, entry), { recursive: true });
        }
    }
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
    if (!isSC(content)) {
        continue;
    }
    wanted.add(entry);
    for (const [from, to] of LINK_MAP) {
        content = content.replaceAll(from, to);
    }
    content = linksFromRoot(keepOnlySC(content), pageUrl("/language/", entry));
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

// Bring the SC property-types reference pages into the manual, where they are
// served under reference/property-types/ (see astro.config.mjs). Only pages
// that opt in with `SC: true` are brought over, just like the specification
// chapters above.
syncDir(
    join(here, "../../astro/src/content/docs/reference/property-types"),
    join(here, "../src/content/docs/reference/property-types"),
    (content) => isSC(content),
    (content, entry) =>
        linksFromRoot(keepOnlySC(content), pageUrl("/reference/property-types/", entry)),
);

console.log(`Synced language specification from ${source}`);
