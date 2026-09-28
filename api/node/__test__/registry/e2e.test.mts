// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// End-to-end test of the slint-ui / slint-ui-dev npm distribution against a local
// (synthetic) Verdaccio registry: publish the tarballs with the same `publishAll`
// routine the real publish uses, then install them as a real consumer would (npm
// and pnpm) and verify that:
//   * `npm i slint-ui` automatically installs the matching platform binary
//     package, slint-ui loads, and only the release features are present,
//   * a simple consumer TypeScript project type-checks against slint-ui's types,
//   * with slint-ui-dev installed, the dev binary (system-testing/mcp) is loaded
//     only when requested via SLINT_MCP_PORT / SLINT_TEST_SERVER, and otherwise
//     the lean release binary is kept,
//   * a version mismatch warns and falls back to the release binary, and
//   * importing slint-ui-dev directly throws.
//
// Two modes:
//   * Gate (publish workflow): SLINT_E2E_TGZ_DIR/_VERSION/_TAG point at the real,
//     already-built all-platform tarballs — see publish_npm_package.yaml.
//   * Dev (local): builds and packs the host packages on the fly under a
//     throwaway version (needs `pnpm build` + `pnpm build:debug` in api/node).
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
    existsSync,
    mkdirSync,
    mkdtempSync,
    readFileSync,
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
    packDevMeta,
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
// A deliberately different version, written into the installed slint-ui-dev to
// trigger binding.cjs's version-mismatch fallback.
const MISMATCH_VERSION = "0.0.0-mismatch";
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
    // Drop any case variant of NPM_CONFIG_USERCONFIG: setup-node sets it to
    // its own .npmrc, which would shadow the one we set below (npm/pnpm
    // lowercase env keys when reading config).
    const env: NodeJS.ProcessEnv = {};
    for (const [key, value] of Object.entries(process.env)) {
        if (key.toLowerCase() !== "npm_config_userconfig") {
            env[key] = value;
        }
    }
    env.npm_config_userconfig = join(work, ".npmrc");
    return env;
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
        // before the per-platform npm-* dirs would land in its tarball).
        const binary = readdirSync(nodeDir).find((f) =>
            /^slint-ui\..+\.node$/.test(f),
        );
        assert.ok(
            binary,
            "host slint-ui.<target>.node not built (run `pnpm build`)",
        );
        const hostTarget = binary.replace(/^slint-ui\.(.+)\.node$/, "$1");
        assert.ok(
            existsSync(join(nodeDir, `slint-ui-dev.${hostTarget}.node`)),
            "host dev binary not built (run `pnpm build:debug`)",
        );
        const targets = [hostTarget];
        tgzDir = join(work, "tgz");
        mkdirSync(tgzDir, { recursive: true });
        await run("npm", ["pkg", "set", `version=${VERSION}`], {
            cwd: nodeDir,
        });
        setMainBinaryDeps({ version: VERSION, targets });
        await run("pnpm", ["pack", "--pack-destination", tgzDir], {
            cwd: nodeDir,
        });
        for (const [config, binary] of [
            ["binaries.json", "slint-ui"],
            ["binaries-dev.json", "slint-ui-dev"],
        ] as const) {
            await packBinary({
                config,
                target: hostTarget,
                binary,
                dest: tgzDir,
            });
        }
        await packDevMeta({ version: VERSION, dest: tgzDir, targets });
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
        // manifests the shared packing functions edit in place.
        rmSync(join(nodeDir, "npm-slint-ui"), { recursive: true, force: true });
        rmSync(join(nodeDir, "npm-slint-ui-dev"), {
            recursive: true,
            force: true,
        });
        rmSync(join(nodeDir, "dev-package", "rust-module-dev.cjs"), {
            force: true,
        });
        rmSync(join(nodeDir, "dev-package", "LICENSE.md"), { force: true });
        execFile("git", [
            "-C",
            repoRoot,
            "checkout",
            "--",
            "api/node/package.json",
            "api/node/dev-package/package.json",
        ]);
    }
});

async function consumerProject(name: string): Promise<string> {
    const dir = join(work, `consumer-${name}`);
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    cpSync(join(work, ".npmrc"), join(dir, ".npmrc"));
    for (const fixture of [
        "assert_load.cts",
        "assert_features.cts",
        "assert_guard.cts",
    ]) {
        cpSync(join(here, fixture), join(dir, fixture));
    }
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

// Each of these env vars independently makes binding.cjs prefer the dev binary —
// they are what actually activate MCP / system testing at runtime. assert_features
// only loads slint-ui and queries build_features(), so the value is never parsed.
const ACTIVATION = {
    SLINT_MCP_PORT: "65000",
    SLINT_TEST_SERVER: "127.0.0.1:65000",
} as const;
type Activation = keyof typeof ACTIVATION;

// Run assert_features.cts. `activateVia` requests the dev binary the same way a
// real MCP / system-testing run does (via that environment variable), so
// binding.cjs loads slint-ui-dev rather than the release binary; `wantDev` is what
// the fixture then expects. Without it, neither variable is set.
function assertFeatures(
    dir: string,
    wantDev: boolean,
    activateVia?: Activation,
) {
    return run("node", [...stripArgs, "./assert_features.cts"], {
        cwd: dir,
        env: {
            ...process.env,
            WANT_DEV: wantDev ? "1" : "",
            SLINT_MCP_PORT: "",
            SLINT_TEST_SERVER: "",
            ...(activateVia ? { [activateVia]: ACTIVATION[activateVia] } : {}),
        },
    });
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
        const dir = await consumerProject(`${pm}-prod`);
        const { stdout, stderr } = await install(pm, dir, [
            `slint-ui@${VERSION}`,
        ]);
        // pnpm >= 10 ignores dependency build scripts and (in newer versions)
        // fails the install over them; slint-ui must not have any.
        assert.ok(
            !`${stdout}${stderr}`.includes("Ignored build scripts"),
            "the install must not trip over ignored build scripts",
        );
        // The matching platform binary package is installed automatically…
        assert.ok(
            await binaryInstalled(dir),
            "no @slint-ui/slint-ui-binary-* was installed",
        );
        // …slint-ui loads (assert_load.cts throws/exits non-zero otherwise)…
        await run("node", [...stripArgs, "./assert_load.cts"], { cwd: dir });
        // …with only the release features, and slint-ui-dev is not pulled in.
        await assertFeatures(dir, false);
        assert.ok(
            !existsSync(join(dir, "node_modules", "slint-ui-dev")),
            "slint-ui-dev must not be installed in a production install",
        );
    });

    test(`${pm}: the dev binary loads only when MCP/system testing is requested`, async () => {
        const dir = await consumerProject(`${pm}-dev`);
        await install(pm, dir, [`slint-ui@${VERSION}`]);
        await install(
            pm,
            dir,
            pm === "npm"
                ? ["--save-dev", `slint-ui-dev@${VERSION}`]
                : ["-D", `slint-ui-dev@${VERSION}`],
        );

        // Cover both activation variables across the two package managers.
        const activateVia: Activation =
            pm === "npm" ? "SLINT_TEST_SERVER" : "SLINT_MCP_PORT";

        // Installed but not requested: still the lean release binary.
        await assertFeatures(dir, false);
        // Requested via the environment: the dev binary is picked up.
        await assertFeatures(dir, true, activateVia);

        // Importing slint-ui-dev directly must throw.
        await run("node", [...stripArgs, "./assert_guard.cts"], { cwd: dir });

        // A version mismatch warns and falls back to the release binary (only
        // reached when the dev binary was requested in the first place).
        const { stdout } = await run(
            "node",
            [
                "-e",
                'process.stdout.write(require.resolve("slint-ui-dev/package.json"))',
            ],
            { cwd: dir },
        );
        const manifest = JSON.parse(readFileSync(stdout.trim(), "utf8"));
        manifest.version = MISMATCH_VERSION;
        writeFileSync(stdout.trim(), JSON.stringify(manifest));

        const { stderr } = await assertFeatures(dir, false, activateVia);
        // The warning names both versions and points at the fix; assert all of it.
        const escape = (v: string) => v.replaceAll(".", "\\.");
        assert.match(
            stderr,
            new RegExp(
                `\\[slint-ui\\] Ignoring slint-ui-dev ${escape(MISMATCH_VERSION)}: ` +
                    `it does not match slint-ui ${escape(VERSION)}\\. ` +
                    `Install slint-ui-dev@${escape(VERSION)}`,
            ),
        );
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
    // The native binary packages are large (especially debug builds), so lift the
    // default 10mb body limit. @slint-ui/*, slint-ui and slint-ui-dev are served
    // locally; everything else is proxied to npmjs.
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
  'slint-ui-dev':
    access: $all
    publish: $all
  '**':
    access: $all
    publish: $all
    proxy: npmjs
log: { type: stdout, format: pretty, level: warn }
`;
}
