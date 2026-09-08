// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

const projectRoot = resolve(new URL("..", import.meta.url).pathname);
const distDir = resolve(projectRoot, "dist-replacement");
const expectedFiles = [
    "browser.html",
    "code.js",
    "manifest.json",
    "ui.html",
    "THIRD_PARTY_NOTICES.txt",
    "dependencies.json",
    "provenance.json",
].sort();
const actualFiles = (await readdir(distDir)).sort();
if (JSON.stringify(actualFiles) !== JSON.stringify(expectedFiles)) {
    throw new Error(
        `Unexpected dist files. Expected ${expectedFiles.join(", ")}; got ${actualFiles.join(", ")}`,
    );
}

const manifest = JSON.parse(
    await readFile(resolve(distDir, "manifest.json"), "utf8"),
);
if (manifest.main !== "code.js" || manifest.ui !== "ui.html") {
    throw new Error(
        "Manifest does not point to the self-contained plugin files",
    );
}
if (manifest.documentAccess !== "dynamic-page") {
    throw new Error("Manifest must use dynamic-page document access");
}
if (
    !Array.isArray(manifest.networkAccess?.allowedDomains) ||
    manifest.networkAccess.allowedDomains.length !== 1 ||
    manifest.networkAccess.allowedDomains[0] !== "none"
) {
    throw new Error("Manifest must disable network access");
}

const ui = await readFile(resolve(distDir, "ui.html"), "utf8");
const browser = await readFile(resolve(distDir, "browser.html"), "utf8");
if (ui !== browser) {
    throw new Error(
        "Figma UI and browser harness must use the same generated document",
    );
}
for (const marker of [
    "<style>",
    "<script>",
    "WebAssembly",
    "compile_from_string",
    "light-slint",
    "dark-slint",
    "source.slint",
    "getLastOnigError",
    "Roboto Mono",
    "counter(step)",
    'id="copy-button"',
]) {
    if (!ui.includes(marker)) {
        throw new Error(`Self-contained UI is missing ${marker}`);
    }
}
for (const [pattern, label] of [
    [/<script\b[^>]*\bsrc\s*=/i, "external script"],
    [/<link\b[^>]*(?:href|as|rel)\s*=/i, "external stylesheet or preload"],
    [/<style\b[^>]*\bsrc\s*=/i, "external style"],
    [
        /<(?:img|iframe|object|embed|source)\b[^>]*(?:src|data|href)\s*=/i,
        "external asset",
    ],
    [/@import\b/i, "CSS import"],
    [/url\(\s*["']?(?:https?:|\/\/)/i, "external CSS URL"],
]) {
    if (pattern.test(ui)) {
        throw new Error(`Self-contained UI contains ${label}`);
    }
}
for (const forbidden of [
    "localStorage",
    "sessionStorage",
    "indexedDB",
    "clientStorage",
    "request-reload",
]) {
    if (ui.includes(forbidden)) {
        throw new Error(
            `Self-contained UI contains forbidden browser integration ${forbidden}`,
        );
    }
}

const code = await readFile(resolve(distDir, "code.js"), "utf8");
for (const marker of [
    "showUI",
    "ui-ready",
    "preview-capture",
    "preview-clear",
    "preview-complete",
    "preview-finalized",
    "pin-state",
    "pin-selection",
    "pin-change",
]) {
    if (!code.includes(marker)) {
        throw new Error(`Plugin sandbox bundle is missing ${marker}`);
    }
}
console.log("Validated self-contained Figma plugin bundle");
if (
    !code.includes("function createSourceNormalizer(") ||
    !code.includes("function convertSnapshot(")
)
    throw new Error("Native codegen conversion is missing from the sandbox");
if (
    !ui.includes("function createSourceNormalizer(") ||
    !ui.includes("function convertSnapshot(")
)
    throw new Error("Offline conversion worker is missing from the UI bundle");

if (ui.includes('id="timing-panel"'))
    throw Error("Production performance UI must be absent");
const devUi = await readFile(resolve(projectRoot, "dist-replacement-dev/ui.html"), "utf8");
if (!devUi.includes('id="timing-panel"'))
    throw Error("Development performance UI is missing");
