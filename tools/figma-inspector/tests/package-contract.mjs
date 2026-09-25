// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import JSZip from "jszip";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const { version } = JSON.parse(
    await readFile(resolve(projectRoot, "package.json"), "utf8"),
);
const runtimePin = JSON.parse(
    await readFile(resolve(projectRoot, "runtime-pin.json"), "utf8"),
);
const packageDir = `Figma to Slint_${version}`;
const archivePath = resolve(projectRoot, "zip", "figma-plugin.zip");
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

const bytes = await readFile(archivePath);
const archive = await JSZip.loadAsync(bytes);
const prefix = `${packageDir}/`;
const provenance = JSON.parse(
    await archive.file(`${packageDir}/provenance.json`).async("string"),
);
if (
    provenance.channel !== "nightly" ||
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

assert.equal(provenance.channel, "nightly");
assert.equal(provenance.repository, "https://github.com/slint-ui/slint.git");
assert.equal(provenance.manifest, "api/wasm-interpreter/Cargo.toml");
assert.match(provenance.revision, /^[a-f0-9]{40}$/);
assert.equal(provenance.revision, runtimePin.revision);
assert.equal(provenance.version, runtimePin.version);
const dependencies = JSON.parse(
    await archive.file(`${prefix}dependencies.json`).async("string"),
);
assert.equal(
    dependencies.packages.find(
        (pkg) =>
            pkg.ecosystem === "cargo" && pkg.name === "slint-wasm-interpreter",
    )?.version,
    runtimePin.version,
);
assert.equal(manifest.id, "1474418299182276871");
assert.deepEqual(manifest.networkAccess.allowedDomains, ["none"]);
assert.ok(
    (await archive.file("readme.txt").async("string")).includes(
        "not a Community release",
    ),
);
for (const field of ["cargoTargetDir", "branch", "materializedAt", "cacheHit"])
    assert.equal(field in provenance, false);
const sums = JSON.parse(
    await archive.file(`${prefix}SHA256SUMS.json`).async("string"),
);
const files = Object.values(archive.files)
    .filter((entry) => !entry.dir)
    .map((entry) => entry.name);
assert.deepEqual(
    Object.keys(sums).sort(),
    files.filter((name) => name !== `${prefix}SHA256SUMS.json`).sort(),
);
for (const [name, hash] of Object.entries(sums))
    assert.equal(
        createHash("sha256")
            .update(await archive.file(name).async("nodebuffer"))
            .digest("hex"),
        hash,
    );
function rejects(args, pattern, env = process.env) {
    assert.throws(
        () =>
            execFileSync(
                process.execPath,
                ["scripts/package-plugin.mjs", ...args],
                { env, stdio: "pipe" },
            ),
        (error) => pattern.test(String(error.stderr)),
    );
}
rejects(["--nightly", "--test"], /Choose exactly one packaging mode/);
rejects(["--release-please"], /Choose exactly one packaging mode/);
rejects(["--nightly"], /FIGMA_PLUGIN_ID must be an assigned/, {
    ...process.env,
    FIGMA_PLUGIN_ID: "000000000000000000",
});
assert.deepEqual(await readFile("zip/figma-plugin.zip"), bytes);
console.log(
    "Validated nightly identity, portable provenance, ZIP checksums and packaging-mode isolation",
);
