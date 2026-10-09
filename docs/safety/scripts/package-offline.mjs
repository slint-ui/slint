// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Turns the built site in `dist/` into a copy in `dist-offline/` that a browser
// opens straight from disk, and zips it.
//
// Browsers resolve root-relative URLs against the file system root on `file://`,
// don't map a directory to its `index.html`, and refuse to load module scripts
// (Chrome and Firefox treat every file as its own origin). So this rewrites
// every root-relative URL to a relative one ending in `index.html`, and bundles
// each page's module scripts into one classic deferred script.
// Pagefind fetches its index at runtime, which `file://` blocks, so the search
// box is hidden.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "dist");
const name = "slint-sc-safety-manual";
const out = path.join(root, "dist-offline", name);
const zip = path.join(root, "dist-offline", `${name}.zip`);

// Generated elsewhere, already using relative URLs.
const untouched = new Set(["api", "coverage"]);

fs.rmSync(out, { recursive: true, force: true });
fs.cpSync(src, out, { recursive: true });
fs.rmSync(path.join(out, "pagefind"), { recursive: true, force: true });

const scratch = fs.mkdtempSync(path.join(out, ".scripts-"));

function* walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (dir === out && untouched.has(entry.name)) continue;
            yield* walk(p);
        } else {
            yield p;
        }
    }
}

/** Maps a root-relative URL to one relative to `fromDir`, pointing at a file. */
function relativize(url, fromDir) {
    if (!url.startsWith("/") || url.startsWith("//")) return url;
    const m = url.match(/^([^?#]*)([?#].*)?$/);
    let target = path.join(out, decodeURI(m[1]));
    if (m[1].endsWith("/") || (fs.existsSync(target) && fs.statSync(target).isDirectory())) {
        target = path.join(target, "index.html");
    }
    const rel = path.relative(fromDir, target).split(path.sep).join("/");
    return encodeURI(rel) + (m[2] ?? "");
}

function rewriteCssUrls(css, fromDir) {
    return css.replace(/url\((["']?)(\/[^)"']*)\1\)/g, (_, q, u) => `url(${q}${relativize(u, fromDir)}${q})`);
}

const bundles = new Map();

/** Bundles the module scripts of one page, in document order, into one file. */
async function bundle(scripts) {
    const entry = scripts
        .map((s) => {
            if (s.src) return `import ${JSON.stringify(path.join(out, s.src))};`;
            const file = path.join(scratch, `${createHash("sha256").update(s.code).digest("hex").slice(0, 16)}.js`);
            fs.writeFileSync(file, s.code);
            return `import ${JSON.stringify(file)};`;
        })
        .join("\n");
    const key = createHash("sha256").update(entry).digest("hex").slice(0, 16);
    if (!bundles.has(key)) {
        bundles.set(key, (async () => {
            await esbuild.build({
                stdin: { contents: entry, resolveDir: out, loader: "js" },
                bundle: true,
                format: "iife",
                minify: true,
                outfile: path.join(out, "_astro", `offline-${key}.js`),
                logLevel: "warning",
                // Raised by code in third-party chunks, such as mermaid.
                logOverride: { "equals-negative-zero": "silent" },
                plugins: [{
                    name: "site-root",
                    setup(build) {
                        build.onResolve({ filter: /^\// }, (args) =>
                            fs.existsSync(args.path) ? { path: args.path } : { path: path.join(out, args.path) },
                        );
                        // Vite's preload helper adds `<link rel=modulepreload>` for chunks that
                        // are now part of the bundle, and `file://` blocks them.
                        build.onLoad({ filter: /\/preload-helper\.[^/]*\.js$/ }, (args) => {
                            const name = fs.readFileSync(args.path, "utf8").match(/export\{\w+ as (\w+)\}/)?.[1];
                            if (!name) throw new Error(`Unexpected Vite preload helper in ${args.path}`);
                            return { contents: `export const ${name} = (load) => load();` };
                        });
                    },
                }],
            });
        })());
    }
    await bundles.get(key);
    return `/_astro/offline-${key}.js`;
}

const hideSearch = "<style>site-search{display:none!important}</style>";

for (const file of [...walk(out)]) {
    const dir = path.dirname(file);
    if (file.endsWith(".css")) {
        fs.writeFileSync(file, rewriteCssUrls(fs.readFileSync(file, "utf8"), dir));
        continue;
    }
    if (!file.endsWith(".html")) continue;

    let html = fs.readFileSync(file, "utf8");

    const scripts = [];
    html = html.replace(/<script type="module"( src="([^"]*)")?>([\s\S]*?)<\/script>/g, (_, _a, src, code) => {
        scripts.push(src ? { src } : { code });
        return "";
    });
    if (scripts.length) {
        const bundled = await bundle(scripts);
        html = html.replace("</body>", `<script defer src="${bundled}"></script></body>`);
    }

    html = html.replace("</head>", `${hideSearch}</head>`);
    html = html.replace(/\b(href|src|action)="([^"]*)"/g, (_, attr, url) => `${attr}="${relativize(url, dir)}"`);
    html = html.replace(/\bsrcset="([^"]*)"/g, (_, set) =>
        `srcset="${set.split(",").map((c) => c.trim().replace(/^\S+/, (u) => relativize(u, dir))).join(", ")}"`,
    );
    html = html.replace(/(content="\d+;\s*url=)([^"]*)"/gi, (_, pre, url) => `${pre}${relativize(url, dir)}"`);
    html = html.replace(/\bstyle="([^"]*)"/g, (_, css) => `style="${rewriteCssUrls(css, dir)}"`);
    html = html.replace(/<style>([\s\S]*?)<\/style>/g, (_, css) => `<style>${rewriteCssUrls(css, dir)}</style>`);

    fs.writeFileSync(file, html);
}

fs.rmSync(scratch, { recursive: true });

fs.rmSync(zip, { force: true });
execFileSync("zip", ["-qr", zip, name], { cwd: path.dirname(out) });
console.log(`Wrote ${path.relative(process.cwd(), zip)}`);
