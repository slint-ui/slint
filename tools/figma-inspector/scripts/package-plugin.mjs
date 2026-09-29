// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import JSZip from "jszip";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
if (args.some((arg) => arg !== "--nightly") || args.length > 1)
    throw Error(
        "Choose exactly one packaging mode: release (no flag) or --nightly",
    );
const nightly = args.includes("--nightly");
const channel = nightly ? "nightly" : "release";
const distDir = resolve(projectRoot, "dist");
const zipDir = resolve(projectRoot, "zip");
const { version } = JSON.parse(
    await readFile(resolve(projectRoot, "package.json"), "utf8"),
);
const packageDir = `Figma to Slint_${version}`;
const outputPath = resolve(
    zipDir,
    nightly ? "figma-plugin.zip" : `${packageDir}.zip`,
);
const placeholderPluginId = "000000000000000000";
const pluginId = process.env.FIGMA_PLUGIN_ID ?? "1474418299182276871";

if (
    pluginId === undefined ||
    !/^\d{10,}$/.test(pluginId) ||
    pluginId === placeholderPluginId ||
    pluginId === "123456789012345678"
) {
    throw new Error(
        "FIGMA_PLUGIN_ID must be an assigned Figma plugin ID; the placeholder is not allowed for ZIP packaging",
    );
}

const manifest = JSON.parse(
    await readFile(resolve(distDir, "manifest.json"), "utf8"),
);
manifest.id = pluginId;
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
zip.file(
    `${packageDir}/manifest.json`,
    JSON.stringify(manifest, null, 4) + "\n",
);
zip.file(`${packageDir}/code.js`, code);
zip.file(`${packageDir}/ui.html`, ui);
for (const file of ["THIRD_PARTY_NOTICES.txt", "dependencies.json"])
    zip.file(`${packageDir}/${file}`, await readFile(resolve(distDir, file)));
const provenance = JSON.parse(
    await readFile(resolve(distDir, "provenance.json"), "utf8"),
);
provenance.channel = channel;
zip.file(
    `${packageDir}/provenance.json`,
    JSON.stringify(provenance, null, 4) + "\n",
);
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
