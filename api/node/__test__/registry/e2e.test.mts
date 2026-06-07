// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// End-to-end test of the slint-ui npm distribution against a local (synthetic)
// Verdaccio registry: publish the tarballs with the same `publishAll` routine the
// real publish uses, then install them as a real consumer would (npm and pnpm)
// and verify that:
//   * `npm i slint-ui` automatically installs the matching platform binary
//     package and slint-ui loads, and
//   * a simple consumer TypeScript project type-checks against slint-ui's types.
//
// Two modes:
//   * Gate (publish workflow): SLINT_E2E_TGZ_DIR/_VERSION/_TAG point at the real,
//     already-built tarballs — see publish_npm_package.yaml.
//   * Dev (local): builds and packs the host packages on the fly under a
//     throwaway version (needs `pnpm build` in api/node).
//
// Run with `node --test` (Node >= 23 strips types automatically; older Node needs
// --experimental-strip-types).
//
// Note: Verdaccio runs in-process, so every external command must be spawned
// asynchronously — a synchronous spawn would block the event loop and the
// registry could not answer the child's requests.

import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
    cpSync,
    mkdirSync,
    mkdtempSync,
    readdirSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Server } from "node:http";
import {
    NAPI_TARGETS,
    packBinary,
    publishAll,
    run,
    setMainBinaryDeps,
} from "../../scripts/packaging.mts";

const { runServer } = createRequire(import.meta.url)(
    "verdaccio",
) as typeof import("verdaccio");

// Gate mode (set by the publish workflow): publish the real, already-built
// tarballs in SLINT_E2E_TGZ_DIR with the real version/dist-tag. Otherwise
// (local/dev) build the host packages on the fly under a throwaway version.
const providedTgzDir = process.env.SLINT_E2E_TGZ_DIR;
const VERSION = providedTgzDir
    ? (process.env.SLINT_E2E_VERSION ?? "")
    : "0.0.0-e2e";
const PUBLISH_TAG = providedTgzDir ? (process.env.SLINT_E2E_TAG ?? "") : "e2e";
const here = dirname(fileURLToPath(import.meta.url)); // api/node/__test__/registry
const nodeDir = join(here, "..", ".."); // api/node
const repoRoot = join(nodeDir, "..", ".."); // repository root
const stripArgs =
    Number(process.versions.node.split(".")[0]) < 23
        ? ["--experimental-strip-types"]
        : [];

let work = "";
let registry = "";
let server: Server | undefined;

function npmEnv(): NodeJS.ProcessEnv {
    return { ...process.env, npm_config_userconfig: join(work, ".npmrc") };
}

before(async () => {
    assert.ok(VERSION, "version not set");
    work = mkdtempSync(join(tmpdir(), "slint-e2e-"));

    let tgzDir: string;
    if (providedTgzDir) {
        // Gate mode: use the real tarballs assembled by the publish workflow.
        tgzDir = providedTgzDir;
    } else {
        // Dev mode: build and pack the host packages on the fly (main first,
        // before the per-platform npm-* dir would land in its tarball).
        const binary = readdirSync(nodeDir).find((f) =>
            /^slint-ui\..+\.node$/.test(f),
        );
        assert.ok(
            binary,
            "host slint-ui.<target>.node not built (run `pnpm build`)",
        );
        const hostTarget = binary.replace(/^slint-ui\.(.+)\.node$/, "$1");
        tgzDir = join(work, "tgz");
        mkdirSync(tgzDir, { recursive: true });
        await run("npm", ["pkg", "set", `version=${VERSION}`], {
            cwd: nodeDir,
        });
        setMainBinaryDeps({ version: VERSION, targets: [hostTarget] });
        await run("pnpm", ["pack", "--pack-destination", tgzDir], {
            cwd: nodeDir,
        });
        await packBinary({
            config: "binaries.json",
            target: hostTarget,
            binary: "slint-ui",
            dest: tgzDir,
        });
    }

    // In-process Verdaccio: binds 127.0.0.1 directly, no readiness polling.
    const config = join(work, "verdaccio.yaml");
    writeFileSync(config, verdaccioConfig(join(work, "storage")));
    const app = await runServer(config);
    server = await new Promise<Server>((resolve) => {
        const listening = app.listen(0, "127.0.0.1", () => resolve(listening));
    });
    const address = server.address();
    const port = typeof address === "object" && address ? address.port : 0;
    assert.ok(port, "Verdaccio did not bind a port");
    registry = `http://127.0.0.1:${port}/`;
    writeFileSync(
        join(work, ".npmrc"),
        [
            `registry=${registry}`,
            `@slint-ui:registry=${registry}`,
            `//127.0.0.1:${port}/:_authToken=fake`,
            `store-dir=${join(work, "pnpm-store")}`,
            "",
        ].join("\n"),
    );

    // Publish with the same routine the real publish uses, just at this registry.
    await publishAll({
        dir: tgzDir,
        registry,
        tag: PUBLISH_TAG || undefined,
        env: npmEnv(),
    });
});

after(() => {
    server?.close();
    if (work) {
        rmSync(work, { recursive: true, force: true });
    }
    if (!providedTgzDir) {
        // Dev mode only: remove generated packaging artifacts and restore the
        // manifest the shared packing functions edit in place.
        rmSync(join(nodeDir, "npm-slint-ui"), { recursive: true, force: true });
        execFile("git", [
            "-C",
            repoRoot,
            "checkout",
            "--",
            "api/node/package.json",
        ]);
    }
});

async function consumerProject(name: string): Promise<string> {
    const dir = join(work, `consumer-${name}`);
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    cpSync(join(work, ".npmrc"), join(dir, ".npmrc"));
    cpSync(join(here, "assert_load.cts"), join(dir, "assert_load.cts"));
    await run("npm", ["init", "-y"], { cwd: dir, env: npmEnv() });
    return dir;
}

function install(pm: string, dir: string, args: string[]) {
    const cmd =
        pm === "npm"
            ? ["install", "--no-audit", "--no-fund", ...args]
            : ["add", ...args];
    return run(pm, cmd, { cwd: dir, env: npmEnv() });
}

// Whether at least one @slint-ui/slint-ui-binary-<target> got auto-installed for
// the project's slint-ui. Resolving from slint-ui itself (rather than checking a
// fixed node_modules path) works under both npm's hoisted and pnpm's isolated
// node_modules layouts.
async function binaryInstalled(dir: string): Promise<boolean> {
    const script =
        "const {createRequire}=require('node:module');" +
        "const req=createRequire(require.resolve('slint-ui'));" +
        `for(const t of ${JSON.stringify([...NAPI_TARGETS])}){` +
        "try{req.resolve('@slint-ui/slint-ui-binary-'+t+'/package.json');" +
        "process.stdout.write('yes');process.exit(0)}catch{}}" +
        "process.stdout.write('no')";
    const { stdout } = await run("node", ["-e", script], {
        cwd: dir,
        env: npmEnv(),
    });
    return stdout.trim() === "yes";
}

for (const pm of ["npm", "pnpm"] as const) {
    test(`${pm}: installing slint-ui pulls in the binary and loads`, async () => {
        const dir = await consumerProject(pm);
        await install(pm, dir, [`slint-ui@${VERSION}`]);
        // The matching platform binary package is installed automatically…
        assert.ok(
            await binaryInstalled(dir),
            "no @slint-ui/slint-ui-binary-* was installed",
        );
        // …and slint-ui loads (assert_load.cts throws/exits non-zero otherwise).
        await run("node", [...stripArgs, "./assert_load.cts"], { cwd: dir });
    });
}

test("a consumer TypeScript project type-checks against slint-ui", async () => {
    const dir = await consumerProject("typecheck");
    await install("npm", dir, [`slint-ui@${VERSION}`, "typescript"]);
    writeFileSync(
        join(dir, "index.ts"),
        'import * as slint from "slint-ui";\nconst _loadFile: typeof slint.loadFile = slint.loadFile;\n',
    );
    writeFileSync(
        join(dir, "tsconfig.json"),
        JSON.stringify({
            compilerOptions: {
                module: "nodenext",
                moduleResolution: "nodenext",
                noEmit: true,
                skipLibCheck: true,
                types: [],
            },
            files: ["index.ts"],
        }),
    );
    await run("npx", ["tsc", "-p", "tsconfig.json"], {
        cwd: dir,
        env: npmEnv(),
    });
});

function verdaccioConfig(storage: string): string {
    // The native binary packages are large, so lift the default 10mb body limit.
    // @slint-ui/* and slint-ui are served locally; everything else is proxied.
    return `storage: ${storage}
max_body_size: 2gb
uplinks:
  npmjs:
    url: https://registry.npmjs.org/
packages:
  '@slint-ui/*':
    access: $all
    publish: $all
  'slint-ui':
    access: $all
    publish: $all
  '**':
    access: $all
    publish: $all
    proxy: npmjs
log: { type: stdout, format: pretty, level: warn }
`;
}
