// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsCppRoot = join(scriptsDir, "..");
const repoRoot = join(docsCppRoot, "..", "..");
const apiCpp = join(repoRoot, "api", "cpp");
const outFile = join(
    docsCppRoot,
    "src",
    "content",
    "docs",
    "generated",
    "thirdparty.md",
);
const aboutJsonFile = join(
    docsCppRoot,
    "src",
    "content",
    "docs",
    "generated",
    ".thirdparty-about.json",
);

interface AboutCrate {
    name: string;
    version: string;
    repository: string | null;
}

interface AboutLicense {
    name: string;
    id: string;
    text: string;
    used_by: { crate: AboutCrate }[];
}

interface AboutOutput {
    licenses: AboutLicense[];
}

function crateUrl(crate: AboutCrate): string {
    const repo = crate.repository?.trim();
    return repo && repo.length > 0
        ? repo
        : `https://crates.io/crates/${crate.name}`;
}

function renderThirdPartyMarkdown(data: AboutOutput): string {
    const lines: string[] = [
        "---",
        "title: Third-Party Licenses",
        "slug: thirdparty",
        "tableOfContents: false",
        "---",
        "",
    ];

    for (const license of data.licenses) {
        lines.push(
            `### <a id="${license.id}"></a> ${license.name}`,
            "",
            "#### Used by:",
            "",
        );
        for (const { crate } of license.used_by) {
            lines.push(
                `- [${crate.name} ${crate.version}](${crateUrl(crate)})`,
            );
        }
        lines.push(
            "",
            "#### License Text",
            "",
            "```",
            license.text.trimEnd(),
            "```",
            "",
        );
    }

    return `${lines.join("\n")}\n`;
}

mkdirSync(dirname(outFile), { recursive: true });

const result = spawnSync(
    "cargo",
    ["about", "generate", "--format", "json", "-o", aboutJsonFile],
    { cwd: apiCpp, stdio: "inherit" },
);

if (result.status !== 0) {
    process.exit(result.status ?? 1);
}

try {
    const data = JSON.parse(readFileSync(aboutJsonFile, "utf8")) as AboutOutput;
    writeFileSync(outFile, renderThirdPartyMarkdown(data), "utf8");
} finally {
    try {
        unlinkSync(aboutJsonFile);
    } catch {
        // ignore if cargo-about did not write the file
    }
}
