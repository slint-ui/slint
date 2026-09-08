// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { verifyRuntime, runtimePin } from "./runtime-pin.mjs";
import {
    artifactCacheKey,
    artifactHashes,
    artifactSchemaVersion,
    buildRecipe,
    validateSlintArtifact,
} from "./slint-artifact.mjs";
import { execFile } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { constants } from "node:fs";
import {
    access,
    copyFile,
    mkdir,
    mkdtemp,
    readFile,
    readdir,
    rename,
    rm,
    stat,
    writeFile,
} from "node:fs/promises";
import { hostname, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sharedTemporaryRoot = process.platform === "win32" ? tmpdir() : "/tmp";
const slintRoot = verifyRuntime();
const interpreterRoot = resolve(slintRoot, "api/wasm-interpreter");
const manifest = resolve(interpreterRoot, "Cargo.toml");
const outputDir = resolve(
    projectRoot,
    process.env.SLINT_WASM_OUTPUT_DIR ?? ".generated/slint-wasm",
);
const cacheRoot = resolve(
    process.env.SLINT_WASM_CACHE_DIR ??
        join(sharedTemporaryRoot, "slint-figma-plugin-wasm-cache"),
);
const artifactCacheRoot = resolve(cacheRoot, "artifacts");
const cargoTargetDir = resolve(
    process.env.CARGO_TARGET_DIR ??
        join(sharedTemporaryRoot, "slint-figma-plugin-cargo-target"),
);
const cargo = process.env.SLINT_CARGO ?? "cargo";
const wasmPack = process.env.SLINT_WASM_PACK ?? "wasm-pack";
const inputWasm = resolve(
    cargoTargetDir,
    "wasm32-unknown-unknown/release/slint_wasm_interpreter.wasm",
);
const buildEnvironment = {
    ...process.env,
    CARGO_TARGET_DIR: cargoTargetDir,
    CARGO_PROFILE_RELEASE_OPT_LEVEL: String(buildRecipe.rustProfile.optLevel),
    CARGO_PROFILE_RELEASE_LTO: buildRecipe.rustProfile.lto,
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS: String(
        buildRecipe.rustProfile.codegenUnits,
    ),
};

async function runGit(args, options = {}) {
    return execFileAsync("git", ["-C", slintRoot, ...args], {
        encoding: "buffer",
        maxBuffer: 100 * 1024 * 1024,
        ...options,
    });
}

async function sourceState() {
    const [{ stdout: revision }, { stdout: tree }, { stdout: branch }] =
        await Promise.all([
            runGit(["rev-parse", "HEAD"]),
            runGit(["rev-parse", "HEAD^{tree}"]),
            runGit(["branch", "--show-current"]),
        ]);
    const { stdout: statusOutput } = await runGit([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ]);
    const dirty = statusOutput.length > 0;
    return {
        revision: revision.toString("utf8").trim(),
        tree: tree.toString("utf8").trim(),
        branch: branch.toString("utf8").trim(),
        dirty,
    };
}

async function inputWasmHash() {
    return createHash("sha256")
        .update(await readFile(inputWasm))
        .digest("hex");
}

async function compileInputWasm() {
    await execFileAsync(
        cargo,
        [
            "build",
            "--locked",
            "--release",
            "--target",
            buildRecipe.cargoTarget,
            "--manifest-path",
            manifest,
            "--features",
            buildRecipe.features.join(","),
        ],
        {
            cwd: slintRoot,
            env: buildEnvironment,
            maxBuffer: 10 * 1024 * 1024,
        },
    );
    return inputWasmHash();
}

async function validateArtifact(directory, expected) {
    try {
        await validateSlintArtifact(directory, expected);
        return true;
    } catch {
        return false;
    }
}

async function assertSourceState(expected) {
    verifyRuntime();
    const current = await sourceState();
    if (
        current.dirty ||
        current.revision !== expected.revision ||
        current.tree !== expected.sourceTree
    )
        throw Error(
            "The pinned Slint source changed during the build; retry with a clean pinned checkout",
        );
    return current;
}

async function copyDirectory(source, destination) {
    await rm(destination, { recursive: true, force: true });
    await mkdir(destination, { recursive: true });
    for (const entry of await readdir(source, { withFileTypes: true })) {
        const sourcePath = resolve(source, entry.name);
        const destinationPath = resolve(destination, entry.name);
        if (entry.isDirectory()) {
            await copyDirectory(sourcePath, destinationPath);
        } else if (entry.isSymbolicLink()) {
            throw new Error(
                `Cached Slint artifact contains a symlink: ${sourcePath}`,
            );
        } else {
            await copyFile(
                sourcePath,
                destinationPath,
                constants.COPYFILE_FICLONE,
            );
        }
    }
}

async function materializeArtifact(
    artifactDir,
    destination,
    expected,
    cacheHit,
) {
    await mkdir(dirname(destination), { recursive: true });
    const staging = await mkdtemp(
        resolve(dirname(destination), ".slint-wasm-"),
    );
    try {
        await copyDirectory(artifactDir, staging);
        const provenancePath = resolve(staging, "provenance.json");
        const provenance = JSON.parse(await readFile(provenancePath, "utf8"));
        const state = await assertSourceState(expected);
        await writeFile(
            provenancePath,
            `${JSON.stringify(
                {
                    ...provenance,
                    // Build identity is immutable. Only local checkout metadata changes.
                    repository: slintRoot,
                    manifest,
                    branch: state.branch,
                    cacheHit,
                    materializedAt: new Date().toISOString(),
                },
                null,
                4,
            )}\n`,
        );
        await validateSlintArtifact(staging, expected);
        await assertSourceState(expected);
        await rm(destination, { recursive: true, force: true });
        await rename(staging, destination);
    } finally {
        await rm(staging, { recursive: true, force: true });
    }
}

function processIsRunning(pid) {
    try {
        process.kill(pid, 0);
        return true;
    } catch (error) {
        return error?.code !== "ESRCH";
    }
}

async function removeAbandonedLock(lockDir) {
    let lockStats;
    try {
        lockStats = await stat(lockDir);
    } catch (error) {
        if (error?.code === "ENOENT") return true;
        return false;
    }
    const ageMs = Date.now() - lockStats.mtimeMs;
    if (ageMs < 5_000) return false;

    let owner;
    try {
        owner = JSON.parse(
            await readFile(resolve(lockDir, "owner.json"), "utf8"),
        );
    } catch {
        if (ageMs < 60_000) return false;
        await rm(lockDir, { recursive: true, force: true });
        return true;
    }
    if (owner.hostname !== hostname()) return false;
    if (Number.isInteger(owner.pid) && processIsRunning(owner.pid)) {
        return false;
    }
    await rm(lockDir, { recursive: true, force: true });
    return true;
}

async function acquireCacheLock(lockDir) {
    const startedAt = Date.now();
    for (;;) {
        let createdLock = false;
        try {
            await mkdir(lockDir);
            createdLock = true;
            await writeFile(
                resolve(lockDir, "owner.json"),
                `${JSON.stringify({
                    hostname: hostname(),
                    pid: process.pid,
                    nonce: randomUUID(),
                    startedAt: new Date().toISOString(),
                })}\n`,
            );
            return true;
        } catch (error) {
            if (createdLock) {
                await rm(lockDir, { recursive: true, force: true });
            }
            if (error?.code !== "EEXIST") throw error;
        }
        if (await removeAbandonedLock(lockDir)) continue;
        if (Date.now() - startedAt > 30 * 60 * 1_000) {
            throw new Error(
                `Timed out waiting for Slint WASM cache lock ${lockDir}`,
            );
        }
        await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
    }
}

async function buildArtifact(artifactDir, state, expected) {
    const temporaryDir = await mkdtemp(
        resolve(artifactCacheRoot, `.${expected.cacheKey}-`),
    );
    try {
        const wasmPackOptions = [
            "build",
            "--release",
            "--target",
            "web",
            "--out-dir",
            temporaryDir,
            interpreterRoot,
            "--",
            "--locked",
            "--features",
            "console_error_panic_hook",
        ];
        await execFileAsync(wasmPack, wasmPackOptions, {
            cwd: slintRoot,
            env: buildEnvironment,
            maxBuffer: 10 * 1024 * 1024,
        });
        if ((await inputWasmHash()) !== expected.inputWasmHash) {
            throw new Error(
                "The Slint interpreter changed while its WASM artifact was building; run the build again for the new source state",
            );
        }

        const files = await readdir(temporaryDir);
        const glueFile = files.find(
            (file) => file === "slint_wasm_interpreter.js",
        );
        const wasmFile = files.find((file) => file.endsWith("_bg.wasm"));
        if (glueFile === undefined || wasmFile === undefined) {
            throw new Error(
                `wasm-pack output is incomplete: ${files.join(", ")}`,
            );
        }
        if ((await stat(resolve(temporaryDir, wasmFile))).size === 0) {
            throw new Error(`Generated WASM is empty: ${wasmFile}`);
        }
        await writeFile(
            resolve(temporaryDir, "provenance.json"),
            `${JSON.stringify(
                {
                    schemaVersion: artifactSchemaVersion,
                    artifactRevision: state.revision,
                    artifactSourceTree: state.tree,
                    repository: slintRoot,
                    manifest,
                    revision: state.revision,
                    sourceTree: state.tree,
                    branch: state.branch,
                    dirty: state.dirty,
                    inputWasmHash: expected.inputWasmHash,
                    cacheKey: expected.cacheKey,
                    buildRecipe,
                    runtimePin,
                    artifactHashes: await artifactHashes(temporaryDir),
                    toolVersions: expected.toolVersions,
                    cargoTargetDir,
                    generatedAt: new Date().toISOString(),
                },
                null,
                4,
            )}\n`,
        );
        await assertSourceState(expected);
        await validateSlintArtifact(temporaryDir, expected);
        await rm(artifactDir, { recursive: true, force: true });
        await rename(temporaryDir, artifactDir);
    } catch (error) {
        await rm(temporaryDir, { recursive: true, force: true });
        throw error;
    }
}

await access(manifest, constants.R_OK);
const manifestText = await readFile(manifest, "utf8");
if (!/^name\s*=\s*"slint-wasm-interpreter"/m.test(manifestText)) {
    throw new Error(`Unexpected interpreter manifest: ${manifest}`);
}

const state = await sourceState();
const [hash, versions] = await Promise.all([
    compileInputWasm(),
    Promise.all(
        [cargo, "rustc", wasmPack].map(async (tool) =>
            (await execFileAsync(tool, ["--version"])).stdout.trim(),
        ),
    ),
]);
const identity = {
    runtimePin,
    revision: state.revision,
    sourceTree: state.tree,
    buildRecipe,
    inputWasmHash: hash,
    toolVersions: {
        cargo: versions[0],
        rustc: versions[1],
        wasmPack: versions[2],
    },
};
const expected = { ...identity, cacheKey: artifactCacheKey(identity) };
await assertSourceState(expected);
const artifactDir = resolve(artifactCacheRoot, expected.cacheKey);
const lockDir = `${artifactDir}.lock`;
await mkdir(artifactCacheRoot, { recursive: true });

// Readers use the same lock as repairers, including while copying the entry.
await acquireCacheLock(lockDir);
try {
    const cacheHit = await validateArtifact(artifactDir, expected);
    if (!cacheHit) await buildArtifact(artifactDir, state, expected);
    await materializeArtifact(artifactDir, outputDir, expected, cacheHit);
    console.log(
        `${cacheHit ? "Restored cached" : "Packaged"} Slint interpreter from ${expected.revision} into ${outputDir}`,
    );
    console.log(`Shared Cargo target: ${cargoTargetDir}`);
} finally {
    await rm(lockDir, { recursive: true, force: true });
}
