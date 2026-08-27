#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Walks dist/**/*.md and verifies every internal link points to a real file
// in dist/. Catches typos in the linkMap and stale targets after page renames.
//
// Then walks the built pages and verifies that every internal link with an
// anchor points to an id (or legacy `name`) the target page really has. This
// half runs on the HTML because that is where the ids of an imported partial
// and the links written by an .astro component appear; starlight-links-validator
// sees neither, because it reads the markdown files a page is written from.

import { readdir, readFile, stat } from "node:fs/promises";
import { join, relative } from "node:path";
import { unified } from "unified";
import remarkParse from "remark-parse";
import { visit } from "unist-util-visit";

// Must match BASE_PATH in docs/common/src/utils/site-config.ts.
const BASE_PATH = "/docs/";
const DIST = "dist";
const INDEX = "index.html";

const parser = unified().use(remarkParse);

async function* walk(dir, extension) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
        const p = join(dir, entry.name);
        if (entry.isDirectory()) {
            yield* walk(p, extension);
        } else if (entry.isFile() && p.endsWith(extension)) {
            yield p;
        }
    }
}

async function collectLinks(file) {
    const text = await readFile(file, "utf8");
    const tree = parser.parse(text);

    // Reference-style `[text][ref]` links carry the URL on the matching
    // definition node — gather those into a map first.
    const definitions = new Map();
    visit(tree, "definition", (node) => {
        definitions.set(node.identifier, node.url);
    });

    const links = [];
    visit(tree, (node) => {
        if (node.type === "link") {
            links.push({ url: node.url, line: node.position?.start.line });
        } else if (node.type === "linkReference") {
            const url = definitions.get(node.identifier);
            if (url) {
                links.push({ url, line: node.position?.start.line });
            }
        }
    });
    return links;
}

/** The site path a built page is served under, e.g. `reference/colors/`. */
function pageRoute(file) {
    const path = relative(DIST, file);
    return path.endsWith(INDEX) ? path.slice(0, -INDEX.length) : path;
}

const errors = [];

for await (const file of walk(DIST, ".md")) {
    for (const { url, line } of await collectLinks(file)) {
        if (!url.startsWith(BASE_PATH)) {
            continue;
        }
        const path = url.slice(BASE_PATH.length).split("#")[0];
        if (path === "") {
            continue;
        }
        const target = join(DIST, path);
        let ok = false;
        try {
            ok = (await stat(target)).isFile();
        } catch {
            ok = false;
        }
        if (!ok) {
            errors.push({ file, line: line ?? "?", url });
        }
    }
}

// Every anchor a page offers, by the route the page is served under.
const anchors = new Map();
const pages = [];
for await (const file of walk(DIST, ".html")) {
    const text = await readFile(file, "utf8");
    pages.push({ file, text });
    anchors.set(
        pageRoute(file),
        new Set(
            [...text.matchAll(/\s(?:id|name)="([^"]+)"/g)].map(
                (match) => match[1],
            ),
        ),
    );
}

for (const { file, text } of pages) {
    const seen = new Set();
    for (const [, href] of text.matchAll(/\shref="([^"]+)"/g)) {
        // Links that climb out of the base path address another Slint site
        // (the Rust, C++ or Node.js API docs), which this build doesn't hold.
        // A link without a path is an anchor of the page itself, whose ids
        // Starlight generates.
        if (!href.startsWith(BASE_PATH) || href.includes("/../")) {
            continue;
        }
        const [path, anchor] = href.slice(BASE_PATH.length).split("#");
        if (!anchor || path === "") {
            continue;
        }
        // A path this build has no page for is a redirect or an asset; the
        // markdown pass above is what checks that a link resolves at all.
        const target = anchors.get(path.endsWith("/") ? path : `${path}/`);
        if (target && !target.has(anchor) && !seen.has(href)) {
            seen.add(href);
            errors.push({
                file,
                line: "?",
                url: `${BASE_PATH}${path}#${anchor}`,
            });
        }
    }
}

if (errors.length > 0) {
    console.error(`\n${errors.length} broken internal link(s):`);
    for (const e of errors) {
        console.error(`  ${e.file}:${e.line} -> ${e.url}`);
    }
    process.exit(1);
}

console.log("validate-md-links: all internal links and anchors resolve");
