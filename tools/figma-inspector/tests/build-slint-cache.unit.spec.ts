// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFile } from "node:child_process";
import {
    chmod,
    copyFile,
    mkdir,
    mkdtemp,
    readFile,
    rm,
    writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { expect, test } from "vitest";

const execFileAsync = promisify(execFile);
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function git(repository: string, args: string[]) {
    await execFileAsync("git", ["-C", repository, ...args]);
}

test("shares packaged artifacts and Cargo output across worktrees", async () => {
    const temporaryRoot = await mkdtemp(
        join(tmpdir(), "slint-wasm-cache-test-"),
    );
    const buildScript = resolve(
        temporaryRoot,
        "project/scripts/build-slint.mjs",
    );
    await mkdir(dirname(buildScript), { recursive: true });
    for (const name of [
        "build-slint.mjs",
        "runtime-pin.mjs",
        "slint-artifact.mjs",
        "check-slint-artifact.mjs",
    ])
        await copyFile(
            resolve(projectRoot, "scripts", name),
            resolve(dirname(buildScript), name),
        );
    const slintRoot = resolve(temporaryRoot, "slint");
    const interpreterRoot = resolve(slintRoot, "api/wasm-interpreter");
    const cacheRoot = resolve(temporaryRoot, "cache");
    const cargoTargetDir = resolve(cacheRoot, "cargo-target");
    const invocationLog = resolve(temporaryRoot, "wasm-pack-invocations.jsonl");
    const fakeCargo = resolve(temporaryRoot, "cargo.mjs");
    const fakeWasmPack = resolve(temporaryRoot, "wasm-pack.mjs");
    const assertBuildProfile = `
import assert from "node:assert/strict";
assert.equal(process.env.CARGO_PROFILE_RELEASE_OPT_LEVEL, "3");
assert.equal(process.env.CARGO_PROFILE_RELEASE_LTO, "fat");
assert.equal(process.env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS, "1");
`;

    try {
        await mkdir(interpreterRoot, { recursive: true });
        await writeFile(
            resolve(interpreterRoot, "Cargo.toml"),
            '[package]\nname = "slint-wasm-interpreter"\nversion = "1.18.0"\n',
        );
        await mkdir(resolve(interpreterRoot, "src"));
        await writeFile(
            resolve(interpreterRoot, "src/source.txt"),
            "version one\n",
        );
        await writeFile(resolve(slintRoot, "unrelated.txt"), "unchanged\n");
        await git(slintRoot, ["init", "-b", "main"]);
        await git(slintRoot, ["add", "."]);
        await git(slintRoot, [
            "-c",
            "user.name=Cache Test",
            "-c",
            "user.email=cache-test@example.invalid",
            "commit",
            "-m",
            "initial",
        ]);

        await writeFile(
            fakeCargo,
            `#!/usr/bin/env node
if (process.argv.includes("--version")) { console.log("test 0.13.1"); process.exit(0); }
${assertBuildProfile}
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
const targetDir = process.env.CARGO_TARGET_DIR + "/wasm32-unknown-unknown/release";
const output = targetDir + "/slint_wasm_interpreter.wasm";
const temporaryOutput = output + "." + process.pid;
const source = await readFile(${JSON.stringify(resolve(interpreterRoot, "src/source.txt"))});
await mkdir(targetDir, { recursive: true });
await writeFile(temporaryOutput, source);
await rename(temporaryOutput, output);
`,
        );
        await writeFile(
            fakeWasmPack,
            `#!/usr/bin/env node
if (process.argv.includes("--version")) { console.log(process.env.TEST_WASM_PACK_VERSION ?? "test 0.13.1"); process.exit(0); }
${assertBuildProfile}
import { appendFile, mkdir, writeFile } from "node:fs/promises";
if (process.env.TEST_DIRTY_SOURCE === "1") await appendFile(${JSON.stringify(resolve(slintRoot, "unrelated.txt"))}, "changed during build");
const outputIndex = process.argv.indexOf("--out-dir") + 1;
const outputDir = process.argv[outputIndex];
await appendFile(${JSON.stringify(invocationLog)}, JSON.stringify({ cargoTargetDir: process.env.CARGO_TARGET_DIR }) + "\\n");
await new Promise((resolve) => setTimeout(resolve, 200));
await mkdir(outputDir, { recursive: true });
await writeFile(outputDir + "/slint_wasm_interpreter.js", "compile_from_string create_with_existing_window run_event_loop\\n");
await writeFile(outputDir + "/slint_wasm_interpreter.d.ts", "export {};\\n");
await writeFile(outputDir + "/slint_wasm_interpreter_bg.wasm", "wasm");
`,
        );
        await chmod(fakeCargo, 0o755);
        await chmod(fakeWasmPack, 0o755);

        async function runBuild(
            outputName: string,
            toolVersion = "test 0.13.1",
            extraEnv: Record<string, string> = {},
        ) {
            const { stdout } = await execFileAsync("git", [
                "-C",
                slintRoot,
                "rev-parse",
                "HEAD",
            ]);
            await writeFile(
                resolve(temporaryRoot, "project/runtime-pin.json"),
                JSON.stringify(
                    extraEnv.TEST_REORDER_PIN === "1"
                        ? { development: true, revision: stdout.trim() }
                        : { revision: stdout.trim(), development: true },
                ),
            );
            return execFileAsync(process.execPath, [buildScript], {
                env: {
                    ...process.env,
                    CARGO_TARGET_DIR: cargoTargetDir,
                    CARGO_PROFILE_RELEASE_OPT_LEVEL: "z",
                    CARGO_PROFILE_RELEASE_LTO: "thin",
                    CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "16",
                    SLINT_CARGO: fakeCargo,
                    SLINT_REPO: slintRoot,
                    SLINT_WASM_CACHE_DIR: cacheRoot,
                    SLINT_WASM_OUTPUT_DIR: resolve(temporaryRoot, outputName),
                    SLINT_WASM_PACK: fakeWasmPack,
                    TEST_WASM_PACK_VERSION: toolVersion,
                    ...extraEnv,
                },
            });
        }

        async function verifyOutput(outputName: string) {
            const checker = pathToFileURL(
                resolve(dirname(buildScript), "check-slint-artifact.mjs"),
            ).href;
            return execFileAsync(
                process.execPath,
                [
                    "--input-type=module",
                    "--eval",
                    `const { verifyArtifact } = await import(${JSON.stringify(checker)}); await verifyArtifact(${JSON.stringify(resolve(temporaryRoot, outputName))});`,
                ],
                { env: { ...process.env, SLINT_REPO: slintRoot } },
            );
        }
        const invocationCount = async () =>
            (await readFile(invocationLog, "utf8")).trim().split("\n").length;

        const [{ stdout: firstOutput }, { stdout: secondOutput }] =
            await Promise.all([runBuild("worktree-a"), runBuild("worktree-b")]);
        const initialInvocations = (await readFile(invocationLog, "utf8"))
            .trim()
            .split("\n")
            .map((line) => JSON.parse(line));
        expect(initialInvocations).toEqual([
            { cargoTargetDir: cargoTargetDir },
        ]);
        expect(`${firstOutput}${secondOutput}`).toContain(
            "Packaged Slint interpreter",
        );
        expect(`${firstOutput}${secondOutput}`).toContain(
            "Restored cached Slint interpreter",
        );

        const firstProvenance = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "worktree-a/provenance.json"),
                "utf8",
            ),
        );
        const secondProvenance = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "worktree-b/provenance.json"),
                "utf8",
            ),
        );
        expect(secondProvenance.cacheKey).toBe(firstProvenance.cacheKey);
        await verifyOutput("worktree-a");
        await verifyOutput("worktree-b");
        const artifactDirectory = resolve(
            cacheRoot,
            "artifacts",
            firstProvenance.cacheKey,
        );
        const cacheProvenance = JSON.parse(
            await readFile(
                resolve(artifactDirectory, "provenance.json"),
                "utf8",
            ),
        );
        expect(cacheProvenance.artifactRevision).toBe(cacheProvenance.revision);
        expect(cacheProvenance.artifactSourceTree).toBe(
            cacheProvenance.sourceTree,
        );
        expect(firstProvenance.buildRecipe.rustProfile).toEqual({
            optLevel: 3,
            lto: "fat",
            codegenUnits: 1,
        });
        expect(secondProvenance.buildRecipe).toEqual(
            firstProvenance.buildRecipe,
        );
        expect(
            [firstProvenance.cacheHit, secondProvenance.cacheHit].sort(),
        ).toEqual([false, true]);
        expect(firstProvenance.artifactRevision).toBe(firstProvenance.revision);

        const { stdout: cachedOutput } = await runBuild("worktree-c");
        expect(cachedOutput).toContain("Restored cached Slint interpreter");
        expect(
            (await readFile(invocationLog, "utf8")).trim().split("\n"),
        ).toHaveLength(1);

        await runBuild("reordered-pin", "test 0.13.1", {
            TEST_REORDER_PIN: "1",
        });
        expect(await invocationCount()).toBe(1);
        await verifyOutput("reordered-pin");

        // Each invalid cache entry must repair once, even with two builders
        // competing to use it. Both resulting outputs must pass the real checker.
        const corruptions = [
            ...[
                "slint_wasm_interpreter.js",
                "slint_wasm_interpreter.d.ts",
                "slint_wasm_interpreter_bg.wasm",
            ].map((file) => async () => {
                const path = resolve(artifactDirectory, file);
                await writeFile(
                    path,
                    Buffer.concat([
                        await readFile(path),
                        Buffer.from("corruption"),
                    ]),
                );
            }),
            async () => {
                await writeFile(
                    resolve(artifactDirectory, "unexpected.js"),
                    "unexpected",
                );
            },
            async () => {
                await rm(
                    resolve(artifactDirectory, "slint_wasm_interpreter.d.ts"),
                );
            },
            async () => {
                await writeFile(
                    resolve(artifactDirectory, "provenance.json"),
                    "{invalid JSON",
                );
            },
            ...[
                (p: Record<string, unknown>) => {
                    delete p.artifactHashes;
                },
                (p: Record<string, unknown>) => {
                    p.artifactHashes = {
                        ...(p.artifactHashes as Record<string, string>),
                        "slint_wasm_interpreter_bg.wasm": "0".repeat(64),
                    };
                },
                (p: Record<string, unknown>) => {
                    p.cacheKey = "0".repeat(64);
                },
                (p: Record<string, unknown>) => {
                    p.dirty = true;
                },
                (p: Record<string, unknown>) => {
                    p.schemaVersion = 0;
                },
                (p: Record<string, unknown>) => {
                    p.buildRecipe = { profile: "debug" };
                },
                (p: Record<string, unknown>) => {
                    p.artifactRevision = "0".repeat(40);
                },
                (p: Record<string, unknown>) => {
                    p.artifactSourceTree = "0".repeat(40);
                },
                (p: Record<string, unknown>) => {
                    p.runtimePin = { revision: "wrong" };
                },
                (p: Record<string, unknown>) => {
                    p.toolVersions = {
                        cargo: "other",
                        rustc: "other",
                        wasmPack: "other",
                    };
                },
            ].map((mutate) => async () => {
                const path = resolve(artifactDirectory, "provenance.json");
                const p = JSON.parse(await readFile(path, "utf8"));
                mutate(p);
                await writeFile(path, JSON.stringify(p));
            }),
        ];
        for (const corrupt of corruptions) {
            const before = await invocationCount();
            await corrupt();
            // The final checker must reject the same entry as cache lookup.
            await expect(
                verifyOutput(resolve(artifactDirectory)),
            ).rejects.toThrow();
            await Promise.all([runBuild("repair-a"), runBuild("repair-b")]);
            expect(await invocationCount()).toBe(before + 1);
            await verifyOutput("repair-a");
            await verifyOutput("repair-b");
            await runBuild("repair-hit");
            expect(await invocationCount()).toBe(before + 1);
        }
        const beforeToolChange = await invocationCount();
        await runBuild("new-tool", "test 0.14.0");
        const newTool = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "new-tool/provenance.json"),
                "utf8",
            ),
        );
        expect(newTool.cacheKey).not.toBe(firstProvenance.cacheKey);
        expect(await invocationCount()).toBe(beforeToolChange + 1);
        await verifyOutput("new-tool");
        const beforeRevisionChange = await invocationCount();

        await writeFile(resolve(slintRoot, "unrelated.txt"), "changed\n");
        await expect(runBuild("worktree-d")).rejects.toThrow("clean checkout");
        await git(slintRoot, [
            "-c",
            "user.name=Cache Test",
            "-c",
            "user.email=cache-test@example.invalid",
            "commit",
            "-am",
            "unrelated change",
        ]);
        await runBuild("worktree-d");
        const unrelatedProvenance = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "worktree-d/provenance.json"),
                "utf8",
            ),
        );
        expect(unrelatedProvenance.inputWasmHash).toBe(
            firstProvenance.inputWasmHash,
        );
        expect(unrelatedProvenance.cacheKey).not.toBe(firstProvenance.cacheKey);
        await verifyOutput("worktree-d");
        expect(unrelatedProvenance.cacheHit).toBe(false);
        expect(
            (await readFile(invocationLog, "utf8")).trim().split("\n"),
        ).toHaveLength(beforeRevisionChange + 1);

        await writeFile(
            resolve(interpreterRoot, "src/source.txt"),
            "version two\n",
        );
        await expect(runBuild("worktree-e")).rejects.toThrow("clean checkout");
        await git(slintRoot, [
            "-c",
            "user.name=Cache Test",
            "-c",
            "user.email=cache-test@example.invalid",
            "commit",
            "-am",
            "new source",
        ]);
        await runBuild("worktree-e");
        const dirtyProvenance = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "worktree-e/provenance.json"),
                "utf8",
            ),
        );
        await verifyOutput("worktree-e");
        expect(dirtyProvenance.dirty).toBe(false);
        expect(dirtyProvenance.cacheKey).not.toBe(firstProvenance.cacheKey);
        expect(
            (await readFile(invocationLog, "utf8")).trim().split("\n"),
        ).toHaveLength(beforeRevisionChange + 2);

        await runBuild("worktree-f");
        const repeatedDirtyProvenance = JSON.parse(
            await readFile(
                resolve(temporaryRoot, "worktree-f/provenance.json"),
                "utf8",
            ),
        );
        expect(repeatedDirtyProvenance.cacheKey).toBe(dirtyProvenance.cacheKey);
        expect(
            (await readFile(invocationLog, "utf8")).trim().split("\n"),
        ).toHaveLength(beforeRevisionChange + 2);
        // A source change during packaging must fail before replacing the
        // previously verified output. Reverting it permits a clean retry.
        const beforeFailure = await readFile(
            resolve(temporaryRoot, "worktree-f/provenance.json"),
            "utf8",
        );
        await expect(
            runBuild("worktree-f", "test source-change", {
                TEST_DIRTY_SOURCE: "1",
            }),
        ).rejects.toThrow("clean checkout");
        expect(
            await readFile(
                resolve(temporaryRoot, "worktree-f/provenance.json"),
                "utf8",
            ),
        ).toBe(beforeFailure);
        await writeFile(resolve(slintRoot, "unrelated.txt"), "changed\n");
        await verifyOutput("worktree-f");
        await runBuild("worktree-f", "test source-change");
        await verifyOutput("worktree-f");
    } finally {
        await rm(temporaryRoot, { recursive: true, force: true });
    }
}, 60_000);
