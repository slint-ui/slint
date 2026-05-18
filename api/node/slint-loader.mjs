// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Node.js module loader hook for .slint files.
// Enables `import { MainWindow } from "./main.slint"` in JavaScript and TypeScript.
//
// Register with: node --import slint-ui/register app.mjs

import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";

// Resolve the absolute path to slint-ui once, so generated modules
// can import it regardless of where the .slint file lives.
const require_ = createRequire(import.meta.url);
const slintUiPath = pathToFileURL(require_.resolve("slint-ui")).href;

/**
 * Resolve hook: intercept .slint specifiers and resolve them to file URLs.
 */
export function resolve(specifier, context, nextResolve) {
    if (specifier.endsWith(".slint")) {
        const resolved = new URL(specifier, context.parentURL);
        return { url: resolved.href, shortCircuit: true, format: "module" };
    }
    return nextResolve(specifier, context);
}

/**
 * Extract exported component, struct, and enum names from .slint source.
 * Uses simple regex matching — no full parser needed.
 */
function extractExportNames(source) {
    const names = [];
    // Match: export component Foo, export struct Foo, export enum Foo
    // Also handles: export component Foo inherits Window {
    const re =
        /^\s*export\s+(?:component|struct|enum|global)\s+([A-Za-z_][A-Za-z0-9_-]*)/gm;
    let m;
    while ((m = re.exec(source)) !== null) {
        // Normalize kebab-case to snake_case (matching the JS runtime behavior)
        names.push(m[1].replace(/-/g, "_"));
    }
    return names;
}

/**
 * Load hook: generate a JS module that calls slint.loadFile() and
 * re-exports each component/struct/enum by name.
 */
export function load(url, context, nextLoad) {
    if (url.endsWith(".slint")) {
        const filePath = fileURLToPath(url);
        const source = readFileSync(filePath, "utf-8");
        const names = extractExportNames(source);

        const moduleSource = [
            `import { loadFile } from ${JSON.stringify(slintUiPath)};`,
            `const _m = loadFile(new URL(${JSON.stringify(url)}));`,
            ...names.map((n) => `export const ${n} = _m.${n};`),
        ].join("\n");

        return { format: "module", source: moduleSource, shortCircuit: true };
    }
    return nextLoad(url, context);
}
