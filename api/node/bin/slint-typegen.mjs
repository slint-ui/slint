#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { createRequire } from "node:module";
import { parseArgs } from "node:util";

const require = createRequire(import.meta.url);
const napi = require("../binding.cjs");
const { version } = require("../package.json");

const usage = `Usage: slint-typegen [-I <path>]... [-L <library>=<path>]... [-o <file.d.ts>] <file>...

Generates the TypeScript declarations of each .slint file, next to it as <file>.d.ts.
For a slint-project.json, they're those of its entry.

Options:
  -I <path>              Include path for imported .slint files and images
  -L <library>=<path>    Library path for @library imports
  -o <file>              Output file for a single input, named *.d.ts, or - for standard output
  -h, --help             Print this help
  -V, --version          Print the version`;

function fail(message) {
    console.error(`error: ${message}`);
    process.exit(1);
}

let parsed;
try {
    parsed = parseArgs({
        allowPositionals: true,
        options: {
            "include-path": { type: "string", short: "I", multiple: true },
            "library-path": { type: "string", short: "L", multiple: true },
            output: { type: "string", short: "o" },
            help: { type: "boolean", short: "h" },
            version: { type: "boolean", short: "V" },
        },
    });
} catch (e) {
    fail(`${e.message}\n\n${usage}`);
}
const { values, positionals } = parsed;

if (values.help) {
    console.log(usage);
    process.exit(0);
}
if (values.version) {
    console.log(`slint-typegen ${version}`);
    process.exit(0);
}
if (positionals.length === 0) {
    fail(`expected a .slint file\n\n${usage}`);
}
if (values.output !== undefined && positionals.length > 1) {
    fail("-o takes a single input file");
}

const libraryPaths = {};
for (const entry of values["library-path"] ?? []) {
    const separator = entry.indexOf("=");
    if (separator < 0) {
        fail(`'${entry}' isn't a library path, as in -L <library>=<path>`);
    }
    libraryPaths[entry.slice(0, separator)] = entry.slice(separator + 1);
}

const levels = {
    [napi.DiagnosticLevel.Error]: "error",
    [napi.DiagnosticLevel.Warning]: "warning",
    [napi.DiagnosticLevel.Info]: "info",
    [napi.DiagnosticLevel.Note]: "note",
};

for (const input of positionals) {
    let result;
    try {
        result = napi.generateTypescriptDeclarations(
            input,
            values.output,
            values["include-path"] ?? [],
            libraryPaths,
        );
    } catch (e) {
        fail(e.message);
    }

    for (const d of result.diagnostics) {
        const position = d.lineNumber
            ? `:${d.lineNumber}:${d.columnNumber}`
            : "";
        const location = d.fileName ? `${d.fileName}${position}: ` : "";
        console.error(`${location}${levels[d.level]}: ${d.message}`);
    }
    if (result.declarations === undefined) {
        process.exitCode = 1;
    } else if (values.output === "-") {
        process.stdout.write(result.declarations);
    }
}
