// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { dependencyNotices } from "./dependency-notices.mjs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const development = process.env.MODE === "dev";
const distDir = resolve(
    projectRoot,
    process.env.PLUGIN_OUTPUT_DIR ?? (development ? "dist-dev" : "dist"),
);
const pluginId = process.env.FIGMA_PLUGIN_ID ?? "1474418299182276871";

await rm(distDir, { recursive: true, force: true });
await mkdir(distDir, { recursive: true });

const sandboxBuild = await build({
    metafile: true,
    absWorkingDir: projectRoot,
    bundle: true,
    entryPoints: ["src/plugin/main.ts"],
    outfile: resolve(distDir, "code.js"),
    format: "iife",
    platform: "browser",
    target: "es2020",
    sourcemap: false,
});

let uiSource = await readFile(
    resolve(projectRoot, "src/ui/index.html"),
    "utf8",
);
if (!development)
    uiSource = uiSource.replace(
        /<!-- development:start -->[\s\S]*?<!-- development:end -->/g,
        "",
    );
const uiStyle = await readFile(
    resolve(projectRoot, "src/ui/style.css"),
    "utf8",
);
const highlightBuild = await build({
    metafile: true,
    absWorkingDir: projectRoot,
    bundle: true,
    entryPoints: ["src/ui/highlight.worker.ts"],
    format: "iife",
    platform: "browser",
    target: "es2020",
    write: false,
});
const conversionBuild = await build({
    metafile: true,
    absWorkingDir: projectRoot,
    entryPoints: ["src/ui/conversion.worker.ts"],
    bundle: true,
    format: "iife",
    platform: "browser",
    target: "es2020",
    write: false,
});
const uiBuild = await build({
    metafile: true,
    absWorkingDir: projectRoot,
    alias: {
        "slint-wasm-generated": resolve(
            projectRoot,
            "../../api/wasm-interpreter/pkg/slint_wasm_interpreter.js",
        ),
        "slint-wasm-binary": resolve(
            projectRoot,
            "../../api/wasm-interpreter/pkg/slint_wasm_interpreter_bg.wasm",
        ),
    },
    bundle: true,
    define: {
        DEVELOPMENT: JSON.stringify(development),
        CONVERSION_WORKER_SOURCE: JSON.stringify(
            conversionBuild.outputFiles[0].text,
        ),
        HIGHLIGHT_WORKER_SOURCE: JSON.stringify(
            highlightBuild.outputFiles[0].text,
        ),
        "import.meta.url": "undefined",
        "process.env.NODE_ENV": '"production"',
    },
    entryPoints: ["src/ui/index.ts"],
    format: "iife",
    loader: { ".wasm": "binary" },
    minify: false,
    outfile: "ui.js",
    platform: "browser",
    target: "es2020",
    write: false,
});
const uiScript = uiBuild.outputFiles[0].text.replace(
    /<\/script/gi,
    "<\\/script",
);
const uiHtml = `<!doctype html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Slint Preview</title><style>${uiStyle}</style></head>
<body>${uiSource}<script>${uiScript}</script></body>
</html>`;
await writeFile(resolve(distDir, "ui.html"), uiHtml);
await writeFile(resolve(distDir, "browser.html"), uiHtml);

await writeFile(
    resolve(distDir, "manifest.json"),
    `${JSON.stringify(
        {
            name: "Figma to Slint",
            id: pluginId,
            api: "1.0.0",
            main: "code.js",
            ui: "ui.html",
            editorType: ["figma", "dev"],
            capabilities: ["codegen", "vscode"],
            codegenLanguages: [{ label: "Slint", value: "slint" }],
            codegenPreferences: [
                {
                    itemType: "select",
                    propertyName: "useVariables",
                    label: "Use Variables",
                    options: [
                        { label: "Yes", value: "true" },
                        { label: "No", value: "false", isDefault: true },
                    ],
                    includedLanguages: ["slint"],
                },
            ],
            documentAccess: "dynamic-page",
            networkAccess: { allowedDomains: ["none"] },
        },
        null,
        4,
    )}\n`,
);

const notices = await dependencyNotices([
    sandboxBuild.metafile,
    highlightBuild.metafile,
    conversionBuild.metafile,
    uiBuild.metafile,
]);
await writeFile(resolve(distDir, "THIRD_PARTY_NOTICES.txt"), notices.text);
await writeFile(
    resolve(distDir, "dependencies.json"),
    `${JSON.stringify(notices.inventory, null, 4)}\n`,
);
const publicProvenance = {
    channel: process.env.PLUGIN_BUILD_CHANNEL ?? "development",
    repository: "https://github.com/slint-ui/slint.git",
    manifest: "api/wasm-interpreter/Cargo.toml",
    revision: execFileSync("git", ["rev-parse", "HEAD"], {
        cwd: projectRoot,
        encoding: "utf8",
    }).trim(),
    version: JSON.parse(
        await readFile(resolve(projectRoot, "package.json"), "utf8"),
    ).version,
};
await writeFile(
    resolve(distDir, "provenance.json"),
    JSON.stringify(publicProvenance, null, 4) + "\n",
);
