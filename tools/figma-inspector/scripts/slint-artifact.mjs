// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { createHash } from "node:crypto";
import { lstat, readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

export const artifactSchemaVersion = 1;
export const artifactFiles = [
    "slint_wasm_interpreter.js",
    "slint_wasm_interpreter.d.ts",
    "slint_wasm_interpreter_bg.wasm",
];
export const buildRecipe = {
    schemaVersion: 4,
    profile: "release",
    target: "web",
    cargoTarget: "wasm32-unknown-unknown",
    features: ["console_error_panic_hook"],
    rustProfile: { optLevel: 3, lto: "fat", codegenUnits: 1 },
};

// JSON property order must not change the identity of otherwise identical pins.
function canonical(value) {
    if (Array.isArray(value)) return value.map(canonical);
    if (value !== null && typeof value === "object")
        return Object.fromEntries(
            Object.keys(value)
                .sort()
                .map((key) => [key, canonical(value[key])]),
        );
    return value;
}
function equal(a, b) {
    return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}
export function artifactCacheKey(identity) {
    return createHash("sha256")
        .update(
            JSON.stringify(
                canonical({
                    schemaVersion: artifactSchemaVersion,
                    ...identity,
                }),
            ),
        )
        .digest("hex");
}
export async function artifactHashes(directory) {
    const hashes = Object.create(null);
    async function visit(relativeDirectory = "") {
        for (const name of (
            await readdir(resolve(directory, relativeDirectory))
        ).sort()) {
            const relativePath = relativeDirectory
                ? `${relativeDirectory}/${name}`
                : name;
            if (relativePath === "provenance.json") continue;
            const path = resolve(directory, relativePath);
            const info = await lstat(path);
            if (info.isDirectory()) await visit(relativePath);
            else if (info.isFile())
                hashes[relativePath] = createHash("sha256")
                    .update(await readFile(path))
                    .digest("hex");
            else
                throw Error(
                    `Slint artifact contains a non-regular file: ${relativePath}`,
                );
        }
    }
    await visit();
    return hashes;
}

/** Validate both immutable cache entries and their materialized copies. */
export async function validateSlintArtifact(directory, expected) {
    for (const file of [...artifactFiles, "provenance.json"]) {
        const info = await lstat(resolve(directory, file));
        if (!info.isFile() || info.size === 0)
            throw Error(
                `Slint artifact must contain a nonempty regular file: ${file}`,
            );
    }
    const provenance = JSON.parse(
        await readFile(resolve(directory, "provenance.json"), "utf8"),
    );
    if (
        provenance === null ||
        typeof provenance !== "object" ||
        provenance.schemaVersion !== artifactSchemaVersion
    )
        throw Error(
            "Unsupported Slint artifact provenance schema; rebuild with pnpm build:slint",
        );
    if (
        provenance.dirty !== false ||
        provenance.revision !== expected.revision ||
        provenance.artifactRevision !== expected.revision ||
        provenance.sourceTree !== expected.sourceTree ||
        provenance.artifactSourceTree !== expected.sourceTree ||
        !equal(provenance.runtimePin, expected.runtimePin) ||
        !equal(provenance.buildRecipe, buildRecipe)
    )
        throw Error(
            "Slint artifact does not match the clean pinned runtime and build recipe",
        );
    if (
        !/^[a-f0-9]{64}$/.test(provenance.inputWasmHash ?? "") ||
        !["cargo", "rustc", "wasmPack"].every(
            (tool) =>
                typeof provenance.toolVersions?.[tool] === "string" &&
                provenance.toolVersions[tool].trim().length > 0,
        )
    )
        throw Error(
            "Slint artifact is missing its input hash or tool versions",
        );
    const identity = {
        runtimePin: provenance.runtimePin,
        revision: provenance.artifactRevision,
        sourceTree: provenance.artifactSourceTree,
        buildRecipe: provenance.buildRecipe,
        inputWasmHash: provenance.inputWasmHash,
        toolVersions: provenance.toolVersions,
    };
    if (
        provenance.cacheKey !== artifactCacheKey(identity) ||
        (expected.cacheKey !== undefined &&
            provenance.cacheKey !== expected.cacheKey)
    )
        throw Error("Slint artifact cache identity mismatch");
    const hashes = await artifactHashes(directory);
    for (const file of Object.keys(hashes))
        if (provenance.artifactHashes?.[file] !== hashes[file])
            throw Error(
                `Slint artifact hash mismatch: ${file}; rebuild with pnpm build:slint`,
            );
    if (
        !equal(
            Object.keys(hashes).sort(),
            Object.keys(provenance.artifactHashes ?? {}).sort(),
        )
    )
        throw Error("Slint artifact file inventory mismatch");
    const glue = await readFile(
        resolve(directory, "slint_wasm_interpreter.js"),
        "utf8",
    );
    for (const name of [
        "compile_from_string",
        "create_with_existing_window",
        "run_event_loop",
    ])
        if (!glue.includes(name))
            throw Error(`Slint artifact glue is missing ${name}`);
    return provenance;
}
