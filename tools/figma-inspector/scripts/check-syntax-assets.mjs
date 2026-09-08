// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { runtimeRoot } from "./runtime-pin.mjs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

const slintRoot = runtimeRoot;

const assets = [
    {
        label: "Slint grammar",
        source: resolve(
            slintRoot,
            "docs/common/src/utils/slint.tmLanguage.json",
        ),
        vendored: resolve(
            projectRoot,
            "src/ui/syntax-assets/slint.tmLanguage.json",
        ),
    },
    {
        label: "light Slint theme",
        source: resolve(
            slintRoot,
            "tools/figma-inspector/src/components/snippet/light-theme.json",
        ),
        vendored: resolve(projectRoot, "src/ui/syntax-assets/light-theme.json"),
    },
    {
        label: "dark Slint theme",
        source: resolve(
            slintRoot,
            "tools/figma-inspector/src/components/snippet/dark-theme.json",
        ),
        vendored: resolve(projectRoot, "src/ui/syntax-assets/dark-theme.json"),
    },
];

for (const asset of assets) {
    const [source, vendored] = await Promise.all([
        readFile(asset.source),
        readFile(asset.vendored),
    ]);
    if (!source.equals(vendored)) {
        throw new Error(
            `Syntax asset mismatch: ${asset.label} (${asset.vendored}) does not match ${asset.source}`,
        );
    }
}

const grammar = JSON.parse(await readFile(assets[0].vendored, "utf8"));
if (grammar.name !== "slint" || grammar.scopeName !== "source.slint") {
    throw new Error(
        `Slint grammar must be named slint and scoped as source.slint; got name ${JSON.stringify(grammar.name)} and scope ${JSON.stringify(grammar.scopeName)}`,
    );
}

for (const [index, expectedName] of ["light-slint", "dark-slint"].entries()) {
    const theme = JSON.parse(
        await readFile(assets[index + 1].vendored, "utf8"),
    );
    if (theme.name !== expectedName) {
        throw new Error(
            `${assets[index + 1].label} must be named ${expectedName}; got ${JSON.stringify(theme.name)}`,
        );
    }
}

console.log(
    `Validated exact Slint syntax assets from ${slintRoot}: ${assets.map(({ label }) => label).join(", ")}`,
);
