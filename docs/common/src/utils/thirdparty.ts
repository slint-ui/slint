// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
//
// Generate a Starlight "Third-Party Licenses" Markdown page from the Markdown
// emitted by `cargo xtask license`. Shared by the per-language docs sites
// (docs/cpp, docs/nodejs, docs/python), which differ only in the crate
// directory and the output path.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/** Slug of the generated Third-Party Licenses page (its URL is `…/thirdparty/`). */
const THIRDPARTY_SLUG = "thirdparty";

/**
 * Relative link from the page to its own raw-markdown sibling served by the
 * `[...slug].md.ts` endpoint. The page lives at `…/${THIRDPARTY_SLUG}/`, so the
 * sibling is one level up at `…/${THIRDPARTY_SLUG}.md`. The links validator
 * configs exclude this exact link, so keep it as the single source of truth.
 */
export const THIRDPARTY_MD_LINK = `../${THIRDPARTY_SLUG}.md`;

const FRONT_MATTER = [
    "---",
    "title: Third-Party Licenses",
    `slug: ${THIRDPARTY_SLUG}`,
    // Each license is an `h2`; limit the on-page navigation to those so it
    // lists the licenses without the per-license "Used by"/"License Text"
    // subheadings.
    "tableOfContents:",
    "    maxHeadingLevel: 2",
    "---",
    "",
    // Point readers at the raw-markdown sibling of this page so they can drop
    // it into their app's own distribution.
    "You can include this page in your application's distribution by copying" +
        ` its [Markdown source](${THIRDPARTY_MD_LINK}).`,
    "",
    "",
].join("\n");

/**
 * Run `cargo xtask license` in `crateDir` and write the rendered Third-Party
 * Licenses Markdown page to `outFile`. Exits the process if the generator
 * fails.
 */
export function generateThirdPartyMarkdown(options: {
    crateDir: string;
    outFile: string;
}): void {
    const { crateDir, outFile } = options;
    mkdirSync(dirname(outFile), { recursive: true });

    // Let the generator write the Markdown body directly to the output file:
    // the listing easily exceeds spawnSync's default stdout buffer, and
    // inheriting stdio keeps the build log and any error visible.
    const result = spawnSync("cargo", ["xtask", "license", "-o", outFile], {
        cwd: crateDir,
        stdio: "inherit",
    });
    if (result.status !== 0) {
        process.exit(result.status ?? 1);
    }

    // Prepend the Starlight front matter the docs site needs (the body itself
    // is shared verbatim with the binary packages, which must not carry it).
    const body = readFileSync(outFile, "utf8");
    writeFileSync(outFile, FRONT_MATTER + body, "utf8");
}
