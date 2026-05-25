// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore Doxyfile

// Orchestrates the API-reference generation: run Doxygen (XML only) over the
// C++ headers, then convert that XML into Markdown for Starlight. This is the
// C++ analogue of the `starlight-typedoc` plugin used by the Node.js docs.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { DoxygenConverter } from "./lib/doxygen.ts";
import { API_ROOT } from "./lib/slug.ts";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsRoot = join(scriptsDir, "..");
const repoRoot = join(docsRoot, "..", "..");
const xmlDir = join(repoRoot, "target", "cppdocs", "xml");
const contentDocs = join(docsRoot, "src", "content", "docs");

function runDoxygen(): void {
    if (!process.env.SLINT_CPP_GENERATED_INCLUDE) {
        console.warn(
            "warning: SLINT_CPP_GENERATED_INCLUDE is not set. Generate the cbindgen\n" +
                "headers first (e.g. `cargo xtask cppdocs`) and point this variable at\n" +
                "the resulting directory, otherwise Doxygen will miss generated symbols.",
        );
    }
    const result = spawnSync("doxygen", ["Doxyfile"], {
        cwd: docsRoot,
        stdio: "inherit",
    });
    if (result.error) {
        throw new Error(
            "Could not run `doxygen`. Install it (e.g. `apt-get install doxygen`) " +
                "or pre-generate the XML at target/cppdocs/xml.",
        );
    }
    if (result.status !== 0) {
        process.exit(result.status ?? 1);
    }
}

function convert(): void {
    if (!existsSync(join(xmlDir, "index.xml"))) {
        throw new Error(`No Doxygen XML at ${xmlDir} (expected index.xml).`);
    }
    const apiDir = join(contentDocs, API_ROOT);
    rmSync(apiDir, { recursive: true, force: true });
    const pages = new DoxygenConverter(xmlDir).convert();
    for (const page of pages) {
        const file = join(contentDocs, `${page.slug}.md`);
        mkdirSync(dirname(file), { recursive: true });
        writeFileSync(file, page.markdown, "utf8");
    }
    console.log(`Generated ${pages.length} C++ API page(s) into ${apiDir}`);
}

runDoxygen();
convert();
