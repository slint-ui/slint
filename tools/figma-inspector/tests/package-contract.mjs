// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import JSZip from "jszip";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const { version } = JSON.parse(
    await readFile(resolve(projectRoot, "package.json"), "utf8"),
);
const packageDir = `Figma to Slint_${version}`;
const archivePath = resolve(
    projectRoot,
    ".package-test/zip",
    `${packageDir}.zip`,
);
const expectedFiles = [
    `${packageDir}/LICENSE.txt`,
    `${packageDir}/SHA256SUMS.json`,
    `${packageDir}/THIRD_PARTY_NOTICES.txt`,
    `${packageDir}/dependencies.json`,
    `${packageDir}/provenance.json`,
    `${packageDir}/code.js`,
    `${packageDir}/manifest.json`,
    `${packageDir}/ui.html`,
    "readme.txt",
].sort();

const archive = await JSZip.loadAsync(await readFile(archivePath));
const provenance = JSON.parse(
    await archive.file(`${packageDir}/provenance.json`).async("string"),
);
if (
    provenance.channel !== "test" ||
    provenance.repository !== "https://github.com/slint-ui/slint.git" ||
    provenance.manifest !== "api/wasm-interpreter/Cargo.toml" ||
    [
        "cargoTargetDir",
        "branch",
        "cacheHit",
        "materializedAt",
        "generatedAt",
    ].some((key) => key in provenance) ||
    JSON.stringify(provenance).includes(projectRoot)
)
    throw Error("Packaged provenance must not contain local checkout metadata");
const actualFiles = Object.entries(archive.files)
    .filter(([, entry]) => !entry.dir)
    .map(([path]) => path)
    .sort();
if (JSON.stringify(actualFiles) !== JSON.stringify(expectedFiles)) {
    throw new Error(
        `Unexpected ZIP files. Expected ${expectedFiles.join(", ")}; got ${actualFiles.join(", ")}`,
    );
}
if (actualFiles.some((path) => path.endsWith("browser.html"))) {
    throw new Error("The distributable ZIP must not contain browser.html");
}

const manifest = JSON.parse(
    await archive.file(`${packageDir}/manifest.json`).async("string"),
);
if (
    manifest.main !== "code.js" ||
    manifest.ui !== "ui.html" ||
    !archive.file(`${packageDir}/${manifest.main}`) ||
    !archive.file(`${packageDir}/${manifest.ui}`)
) {
    throw new Error(
        "Packaged manifest does not reference packaged plugin files",
    );
}

const readme = await archive.file("readme.txt").async("string");
for (const instruction of [
    "Import plugin from manifest",
    `${packageDir}/manifest.json`,
    "Plugins > Development",
]) {
    if (!readme.includes(instruction)) {
        throw new Error(
            `Packaged readme is missing installation instruction: ${instruction}`,
        );
    }
}

console.log("Validated allowlisted Figma plugin ZIP");

const ui = await archive.file(`${packageDir}/ui.html`).async("string");
if (ui.includes('id="timing-panel"') || ui.includes('id="copy-trace"'))
    throw Error("Development performance UI leaked into package");
