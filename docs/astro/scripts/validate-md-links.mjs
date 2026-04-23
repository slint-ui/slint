#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Walks dist/**/*.md and verifies every internal link points to a real file
// in dist/. Catches typos in the linkMap and stale targets after page renames.
// Anchor fragments are not validated yet — file existence only.

import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { unified } from "unified";
import remarkParse from "remark-parse";
import { visit } from "unist-util-visit";

// Must match BASE_PATH in docs/common/src/utils/site-config.ts.
const BASE_PATH = "/docs/";
const DIST = "dist";

const parser = unified().use(remarkParse);

async function* walk(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
        const p = join(dir, entry.name);
        if (entry.isDirectory()) {
            yield* walk(p);
        } else if (entry.isFile() && p.endsWith(".md")) {
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

const errors = [];

for await (const file of walk(DIST)) {
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

if (errors.length > 0) {
    console.error(`\n${errors.length} broken markdown link(s):`);
    for (const e of errors) {
        console.error(`  ${e.file}:${e.line} -> ${e.url}`);
    }
    process.exit(1);
}

console.log("validate-md-links: all internal markdown links resolve");
