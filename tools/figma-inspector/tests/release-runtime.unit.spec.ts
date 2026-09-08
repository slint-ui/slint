// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFile } from "node:child_process";
import { copyFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { afterAll, beforeAll, expect, test } from "vitest";

const execFileAsync = promisify(execFile);
const revision = "a".repeat(40);
const otherRevision = "b".repeat(40);
const tag = "refs/tags/v1.18.0";
let temporaryRoot: string;
let scriptUrl: string;

beforeAll(async () => {
    temporaryRoot = await mkdtemp(join(tmpdir(), "slint-release-runtime-"));
    const script = join(temporaryRoot, "scripts/runtime-pin.mjs");
    await mkdir(dirname(script));
    await copyFile(
        resolve(
            dirname(fileURLToPath(import.meta.url)),
            "../scripts/runtime-pin.mjs",
        ),
        script,
    );
    await writeFile(
        join(temporaryRoot, "runtime-pin.json"),
        JSON.stringify({
            repository: "https://github.com/slint-ui/slint.git",
            revision,
            version: "1.18.0",
            releaseTag: "v1.18.0",
            development: false,
        }),
    );
    scriptUrl = pathToFileURL(script).href;
});
afterAll(async () => {
    await rm(temporaryRoot, { recursive: true, force: true });
});

// Exercise the real release gate with a deterministic Git boundary. Builtin
// mocking stays inside the child process and never invokes Git or the network.
async function verify(config: Record<string, unknown> = {}) {
    const { stdout } = await execFileAsync(
        process.execPath,
        [
            "--input-type=module",
            "--eval",
            `
        import childProcess from "node:child_process";
        import { syncBuiltinESMExports } from "node:module";
        const config = JSON.parse(process.argv[1]);
        const calls = [];
        childProcess.execFileSync = (command, args, options) => {
            if (command !== "git") throw Error("Unexpected command");
            calls.push({args, timeout: options.timeout, killSignal: options.killSignal,
                prompt: options.env?.GIT_TERMINAL_PROMPT, stdio: options.stdio});
            if (args[0] === "ls-remote") {
                if (config.remoteError) throw Object.assign(new Error("Simulated Git failure"), {code: config.remoteError});
                return config.remote ?? ${JSON.stringify(`${revision}\t${tag}\n`)};
            }
            const operation = args.slice(2).join(" ");
            if (operation === "rev-parse HEAD") return config.head ?? ${JSON.stringify(revision)};
            if (operation === "status --porcelain --untracked-files=all") return config.dirty ? " M Cargo.toml" : "";
            if (operation === "rev-parse refs/tags/v1.18.0^{commit}") {
                if (config.missingLocalTag) throw Error("Unknown revision");
                return config.localTag ?? ${JSON.stringify(revision)};
            }
            throw Error("Unexpected Git operation: " + operation);
        };
        syncBuiltinESMExports();
        const module = await import(process.argv[2]);
        Object.assign(module.runtimePin, config.pin ?? {});
        let result;
        try { module.verifyRuntime(config.release ?? true); result = {ok: true}; }
        catch (error) { result = {ok: false, message: error.message, cause: error.cause?.code}; }
        console.log(JSON.stringify({...result, calls}));
    `,
            JSON.stringify(config),
            scriptUrl,
        ],
        {
            env: {
                ...process.env,
                SLINT_REPO: join(temporaryRoot, "upstream"),
            },
            timeout: 5_000,
        },
    );
    return JSON.parse(stdout);
}

test("accepts an official lightweight tag with a bounded noninteractive lookup", async () => {
    const result = await verify();
    expect(result.ok).toBe(true);
    expect(
        result.calls.find(
            (call: { args: string[] }) => call.args[0] === "ls-remote",
        ),
    ).toEqual({
        args: [
            "ls-remote",
            "--tags",
            "https://github.com/slint-ui/slint.git",
            tag,
            `${tag}^{}`,
        ],
        timeout: 30_000,
        killSignal: "SIGKILL",
        prompt: "0",
        stdio: ["ignore", "pipe", "pipe"],
    });
});

test("uses an annotated tag's peeled commit rather than its tag object", async () => {
    expect(
        (
            await verify({
                remote: `${otherRevision}\t${tag}\n${revision}\t${tag}^{}\n`,
            })
        ).ok,
    ).toBe(true);
    const wrongCommit = await verify({
        remote: `${revision}\t${tag}\n${otherRevision}\t${tag}^{}\n`,
    });
    expect(wrongCommit.ok).toBe(false);
    expect(wrongCommit.message).toContain("Recheck the pin");
});

test.each([
    ["mismatched", `${otherRevision}\t${tag}\n`],
    ["missing", ""],
    ["malformed", "not-a-git-ref"],
    ["unrelated peeled", `${revision}\trefs/tags/unrelated^{}\n`],
])("rejects a %s remote release tag", async (_name, remote) => {
    const result = await verify({ remote });
    expect(result.ok).toBe(false);
    expect(result.message).toContain(
        "Official Slint release tag v1.18.0 is missing or does not match",
    );
});

test.each([
    ["timeout", "ETIMEDOUT", "Timed out after 30 seconds"],
    ["network failure", "ECONNRESET", "Git lookup failed"],
])(
    "fails closed with retry guidance after a %s",
    async (_name, code, message) => {
        const result = await verify({ remoteError: code });
        expect(result.ok).toBe(false);
        expect(result.cause).toBe(code);
        expect(result.message).toContain(message);
        expect(result.message).toContain(
            "Check Git/network access to github.com and retry pnpm zip",
        );
    },
);

test.each([
    [
        "local tag mismatch",
        { localTag: otherRevision },
        "Local Slint release tag",
    ],
    [
        "missing local tag",
        { missingLocalTag: true },
        "Prepare a clean checkout",
    ],
    ["wrong checkout", { head: otherRevision }, "clean checkout"],
    ["dirty checkout", { dirty: true }, "clean checkout"],
    [
        "development pin",
        { pin: { development: true } },
        "official Slint 1.18.0 release pin",
    ],
    [
        "nonofficial repository",
        { pin: { repository: "https://example.invalid/slint.git" } },
        "official Slint 1.18.0 release pin",
    ],
])("rejects %s before any remote lookup", async (_name, config, message) => {
    const result = await verify(config);
    expect(result.ok).toBe(false);
    expect(result.message).toContain(message);
    expect(
        result.calls.some(
            (call: { args: string[] }) => call.args[0] === "ls-remote",
        ),
    ).toBe(false);
});

test("development builds do not require a remote release lookup", async () => {
    const result = await verify({
        release: false,
        pin: { development: true },
        remoteError: "ETIMEDOUT",
    });
    expect(result.ok).toBe(true);
    expect(
        result.calls.some(
            (call: { args: string[] }) => call.args[0] === "ls-remote",
        ),
    ).toBe(false);
});
