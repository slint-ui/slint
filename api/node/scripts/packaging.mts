// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Shared helpers for assembling the slint-ui npm packages, used both by the
// publish workflow (as a CLI, e.g. `node packaging.mts pack-binary …`) and by
// the registry e2e test (which imports the functions directly). This is the
// single source of truth for the set of platform binary packages.
//
// Run with Node's TypeScript type stripping (Node >= 23, or --experimental-strip-types).

import { execFile } from "node:child_process";
import { cpSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const here = dirname(fileURLToPath(import.meta.url)); // api/node/scripts
const nodeDir = join(here, ".."); // api/node
const repoRoot = join(nodeDir, "..", ".."); // repository root

/** The napi-rs platform suffixes for the binary packages. */
export const NAPI_TARGETS = [
    "linux-x64-gnu",
    "linux-arm64-gnu",
    "linux-x64-musl",
    "linux-arm64-musl",
    "darwin-arm64",
    "win32-x64-msvc",
    "win32-arm64-msvc",
] as const;

/** Base npm package names for the release and dev platform binaries. */
export const BASE_BINARY_PACKAGE = "@slint-ui/slint-ui-binary";
export const DEV_BINARY_PACKAGE = "@slint-ui/slint-ui-dev-binary";

/** Full npm package names for a binary base name, e.g. `@slint-ui/slint-ui-binary`. */
export function binaryPackageNames(
    base: string,
    targets: readonly string[] = NAPI_TARGETS,
): string[] {
    return targets.map((target) => `${base}-${target}`);
}

export interface RunResult {
    stdout: string;
    stderr: string;
}

/** Run an external command, capturing its output and throwing a readable error. */
export async function run(
    cmd: string,
    args: string[],
    opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {},
): Promise<RunResult> {
    try {
        const { stdout, stderr } = await execFileAsync(cmd, args, {
            encoding: "utf8",
            maxBuffer: 64 * 1024 * 1024,
            // npm/npx/pnpm are .cmd shims on Windows, which execFile cannot launch
            // directly; run through the shell there. (The args below are package
            // names and ./-relative paths, so no extra quoting is needed.)
            shell: process.platform === "win32",
            ...opts,
        });
        return { stdout, stderr };
    } catch (error) {
        const e = error as { stdout?: string; stderr?: string; code?: number };
        throw new Error(
            `${cmd} ${args.join(" ")} failed (${e.code}):\n${e.stdout}\n${e.stderr}`,
        );
    }
}

function editManifest(
    file: string,
    edit: (manifest: Record<string, any>) => void,
): void {
    const manifest = JSON.parse(readFileSync(file, "utf8"));
    edit(manifest);
    writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
}

/** Pack a single platform's prebuilt native binary into an npm package tarball. */
export async function packBinary(opts: {
    config: string; // napi binaries config (binaries.json / binaries-dev.json)
    target: string; // napi-rs platform suffix, e.g. linux-x64-gnu
    binary: string; // the <name>.<target>.node prefix, e.g. slint-ui / slint-ui-dev
    dest: string; // directory the .tgz is written to
}): Promise<void> {
    const { config, target, binary, dest } = opts;
    const npmDir = `npm-${binary}`;
    await run(
        "npx",
        [
            "napi",
            "create-npm-dirs",
            "--npm-dir",
            `./${npmDir}`,
            "-c",
            `./${config}`,
        ],
        { cwd: nodeDir },
    );
    cpSync(
        join(nodeDir, `${binary}.${target}.node`),
        join(nodeDir, npmDir, target, `${binary}.${target}.node`),
    );
    const pkgDir = join(nodeDir, npmDir, target);
    await run(
        "pnpm",
        [
            "pkg",
            "set",
            "repository.type=git",
            "repository.url=https://github.com/slint-ui/slint",
        ],
        { cwd: pkgDir },
    );
    await run("pnpm", ["pack", "--pack-destination", dest], { cwd: pkgDir });
}

/**
 * Inject the base binary optionalDependencies and the optional slint-ui-dev peer
 * dependency into the main slint-ui package.json. Only the release base binaries
 * are optional dependencies of the main package, so `npm i slint-ui` stays lean;
 * the dev binaries ship via the separate slint-ui-dev package, declared here as
 * an optional peer.
 */
export function setMainBinaryDeps(opts: {
    version: string;
    targets?: readonly string[];
}): void {
    const { version, targets } = opts;
    editManifest(join(nodeDir, "package.json"), (manifest) => {
        manifest.optionalDependencies ??= {};
        for (const pkg of binaryPackageNames(BASE_BINARY_PACKAGE, targets)) {
            manifest.optionalDependencies[pkg] = version;
        }
        manifest.peerDependencies = {
            ...manifest.peerDependencies,
            "slint-ui-dev": version,
        };
        manifest.peerDependenciesMeta = {
            ...manifest.peerDependenciesMeta,
            "slint-ui-dev": { optional: true },
        };
    });
}

/**
 * Assemble and pack the slint-ui-dev meta-package: it ships the dev loader and
 * pulls in the matching platform's dev binary. Requires the dev loader
 * (rust-module-dev.cjs) to be built in api/node.
 */
export async function packDevMeta(opts: {
    version: string;
    dest: string;
    targets?: readonly string[];
}): Promise<void> {
    const { version, dest, targets } = opts;
    const devDir = join(nodeDir, "dev-package");
    cpSync(join(repoRoot, "LICENSE.md"), join(devDir, "LICENSE.md"));
    cpSync(
        join(nodeDir, "rust-module-dev.cjs"),
        join(devDir, "rust-module-dev.cjs"),
    );
    editManifest(join(devDir, "package.json"), (manifest) => {
        manifest.version = version;
        manifest.optionalDependencies ??= {};
        for (const pkg of binaryPackageNames(DEV_BINARY_PACKAGE, targets)) {
            manifest.optionalDependencies[pkg] = version;
        }
        manifest.peerDependencies = {
            ...manifest.peerDependencies,
            "slint-ui": version,
        };
    });
    await run("pnpm", ["pack", "--pack-destination", dest], { cwd: devDir });
}

/**
 * Publish every *.tgz in `dir` to a registry. Used identically against the
 * throwaway Verdaccio registry (the e2e gate) and the real npm registry; only
 * the registry URL (and the ambient auth in env) differ.
 */
export async function publishAll(opts: {
    dir: string;
    registry: string;
    tag?: string;
    env?: NodeJS.ProcessEnv;
}): Promise<void> {
    const { dir, registry, tag, env } = opts;
    const tagArgs = tag ? ["--tag", tag] : [];
    for (const file of readdirSync(dir)) {
        if (!file.endsWith(".tgz")) {
            continue;
        }
        await run(
            "pnpm",
            [
                "publish",
                "--no-git-checks",
                "--access",
                "public",
                ...tagArgs,
                "--registry",
                registry,
                join(dir, file),
            ],
            { cwd: dir, env },
        );
    }
}

async function main(argv: string[]): Promise<void> {
    const [command, ...args] = argv;
    switch (command) {
        case "pack-binary": {
            const [config, target, binary, dest] = args;
            await packBinary({ config, target, binary, dest });
            break;
        }
        case "set-main-deps": {
            const [version, ...targets] = args;
            setMainBinaryDeps({
                version,
                targets: targets.length ? targets : undefined,
            });
            break;
        }
        case "pack-dev-meta": {
            const [version, dest, ...targets] = args;
            await packDevMeta({
                version,
                dest,
                targets: targets.length ? targets : undefined,
            });
            break;
        }
        case "publish-all": {
            const [dir, registry, tag] = args;
            await publishAll({ dir, registry, tag: tag || undefined });
            break;
        }
        default:
            throw new Error(`unknown command: ${command}`);
    }
}

// Run as a CLI when executed directly (rather than imported).
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
    main(process.argv.slice(2)).catch((error) => {
        console.error(error);
        process.exit(1);
    });
}
