// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Module hooks that turn a `.slint` file into an ES module, so that
// `import { MainWindow } from "./main.slint"` works.
//
// Install them with `slint-ui/register`.

import { createRequire } from "node:module";

const slint = createRequire(import.meta.url)("slint-ui");

// Holds each compiled file between the hook that compiles it and the generated
// module that re-exports it, so a file is compiled once.
const compiled = new Map();

/** Hand the compiled module for `url` to the generated module that re-exports it. */
export function takeCompiled(url) {
    const module = compiled.get(url);
    compiled.delete(url);
    return module ?? slint.loadFile(new URL(url));
}

/**
 * Compile the `.slint` file at `url` and return the source of a module that
 * re-exports what it declares. The names come from the compiler rather than
 * from reading the markup, so they match what `loadFile` returns.
 */
export function moduleSource(url) {
    const module = slint.loadFile(new URL(url));
    compiled.set(url, module);
    return [
        `import { takeCompiled } from ${JSON.stringify(import.meta.url)};`,
        `const _module = takeCompiled(${JSON.stringify(url)});`,
        ...Object.getOwnPropertyNames(module).map(
            (name) => `export const ${name} = _module.${name};`,
        ),
    ].join("\n");
}

/** `resolve` hook for `node:module`. */
export function resolve(specifier, context, nextResolve) {
    if (specifier.endsWith(".slint")) {
        return {
            url: new URL(specifier, context.parentURL).href,
            format: "module",
            shortCircuit: true,
        };
    }
    return nextResolve(specifier, context);
}

/** `load` hook for `node:module`. */
export function load(url, context, nextLoad) {
    if (url.endsWith(".slint")) {
        return {
            format: "module",
            source: moduleSource(url),
            shortCircuit: true,
        };
    }
    return nextLoad(url, context);
}
