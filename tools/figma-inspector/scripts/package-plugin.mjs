// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { createHash } from "node:crypto";
import { execFile as execFileCallback } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import JSZip from "jszip";

const execFile = promisify(execFileCallback);
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
if (
    args.some((arg) => !["--test", "--nightly"].includes(arg)) ||
    args.length > 1
)
    throw Error(
        "Choose exactly one packaging mode: release (no flag), --nightly, or --test",
    );
const testing = args.includes("--test");
const nightly = args.includes("--nightly");
const channel = testing ? "test" : nightly ? "nightly" : "release";
const distDir = resolve(projectRoot, testing ? ".package-test/dist" : "dist");
const zipDir = resolve(projectRoot, testing ? ".package-test/zip" : "zip");
const { version } = JSON.parse(
    await readFile(resolve(projectRoot, "package.json"), "utf8"),
);
const packageDir = `Figma to Slint_${version}`;
const outputPath = resolve(
    zipDir,
    nightly ? "figma-plugin.zip" : `${packageDir}.zip`,
);
const placeholderPluginId = "000000000000000000";
const pluginId = testing
    ? "123456789012345678"
    : (process.env.FIGMA_PLUGIN_ID ?? "1474418299182276871");

if (
    pluginId === undefined ||
    !/^\d{10,}$/.test(pluginId) ||
    pluginId === placeholderPluginId ||
    (!testing && pluginId === "123456789012345678")
) {
    throw new Error(
        "FIGMA_PLUGIN_ID must be an assigned Figma plugin ID; the placeholder is not allowed for ZIP packaging",
    );
}

const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
await execFile(pnpm, ["build"], {
    cwd: projectRoot,
    env: {
        ...process.env,
        FIGMA_PLUGIN_ID: pluginId,
        MODE: "production",
        PLUGIN_BUILD_CHANNEL: channel,
        PLUGIN_OUTPUT_DIR: distDir,
    },
    stdio: "inherit",
});

const manifest = await readFile(resolve(distDir, "manifest.json"));
const code = await readFile(resolve(distDir, "code.js"));
const ui = await readFile(resolve(distDir, "ui.html"));

const readme = `Figma to Slint ${version}
Channel: ${channel}
${nightly ? "Development runtime snapshot; not a Community release." : ""}

Install the development plugin in Figma Desktop:

1. Extract this ZIP archive.
2. Open Figma Desktop and choose Plugins > Development > Import plugin from manifest.
3. Select the nested "${packageDir}/manifest.json" file.
4. Run "Figma to Slint" from Plugins > Development.

The plugin is self-contained and does not require network access.
`;

await rm(zipDir, { recursive: true, force: true });
await mkdir(zipDir, { recursive: true });

const zip = new JSZip();
zip.file("readme.txt", readme);
zip.file(`${packageDir}/manifest.json`, manifest);
zip.file(`${packageDir}/code.js`, code);
zip.file(`${packageDir}/ui.html`, ui);
for (const file of [
    "THIRD_PARTY_NOTICES.txt",
    "dependencies.json",
    "provenance.json",
])
    zip.file(`${packageDir}/${file}`, await readFile(resolve(distDir, file)));
zip.file(
    `${packageDir}/LICENSE.txt`,
    await readFile(resolve(projectRoot, "../../LICENSES/MIT.txt")),
);
const hashes = {};
for (const [name, file] of Object.entries(zip.files))
    if (!file.dir)
        hashes[name] = createHash("sha256")
            .update(await file.async("nodebuffer"))
            .digest("hex");
zip.file(`${packageDir}/SHA256SUMS.json`, JSON.stringify(hashes, null, 2));

await writeFile(
    outputPath,
    await zip.generateAsync({
        type: "nodebuffer",
        compression: "DEFLATE",
    }),
);

console.log(`Created ${outputPath}`);
