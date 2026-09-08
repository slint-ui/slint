// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
export const runtimePin = JSON.parse(
    readFileSync(new URL("../runtime-pin.json", import.meta.url), "utf8"),
);
export const runtimeRoot = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    process.env.SLINT_REPO ?? ".generated/slint-source",
);
export function verifyRuntime(release = false) {
    const git = (...args) =>
        execFileSync("git", ["-C", runtimeRoot, ...args], {
            encoding: "utf8",
        }).trim();
    if (
        git("rev-parse", "HEAD") !== runtimePin.revision ||
        git("status", "--porcelain", "--untracked-files=all")
    )
        throw Error(
            "SLINT_REPO must be a clean checkout of runtime-pin.json; the build never modifies that checkout.",
        );
    if (
        release &&
        (runtimePin.development !== false ||
            runtimePin.repository !== "https://github.com/slint-ui/slint.git" ||
            runtimePin.version !== "1.18.0" ||
            runtimePin.releaseTag !== "v1.18.0")
    )
        throw Error(
            "Commercial packaging requires the official Slint 1.18.0 release pin. Development snapshots cannot be published.",
        );
    if (release) {
        let localRevision;
        try {
            localRevision = git(
                "rev-parse",
                `refs/tags/${runtimePin.releaseTag}^{commit}`,
            );
        } catch (cause) {
            throw new Error(
                `Cannot resolve local Slint release tag ${runtimePin.releaseTag}. Prepare a clean checkout of the official release tag and retry pnpm zip.`,
                { cause },
            );
        }
        if (localRevision !== runtimePin.revision)
            throw Error(
                `Local Slint release tag ${runtimePin.releaseTag} does not match runtime-pin.json. Correct the release checkout and pin before packaging.`,
            );
        let output;
        try {
            output = execFileSync(
                "git",
                [
                    "ls-remote",
                    "--tags",
                    runtimePin.repository,
                    `refs/tags/${runtimePin.releaseTag}`,
                    `refs/tags/${runtimePin.releaseTag}^{}`,
                ],
                {
                    encoding: "utf8",
                    timeout: 30_000,
                    killSignal: "SIGKILL",
                    stdio: ["ignore", "pipe", "pipe"],
                    env: { ...process.env, GIT_TERMINAL_PROMPT: "0" },
                },
            );
        } catch (cause) {
            const reason =
                cause?.code === "ETIMEDOUT"
                    ? "Timed out after 30 seconds"
                    : "Git lookup failed";
            throw new Error(
                `${reason} while verifying official Slint tag ${runtimePin.releaseTag}. Check Git/network access to github.com and retry pnpm zip; official-tag verification is required before packaging.`,
                { cause },
            );
        }
        const refs = output
            .trim()
            .split("\n")
            .map((line) => line.split(/\s+/));
        const official =
            refs.find(
                ([, ref]) => ref === `refs/tags/${runtimePin.releaseTag}^{}`,
            ) ??
            refs.find(
                ([, ref]) => ref === `refs/tags/${runtimePin.releaseTag}`,
            );
        if (official?.[0] !== runtimePin.revision)
            throw Error(
                `Official Slint release tag ${runtimePin.releaseTag} is missing or does not match runtime-pin.json. Recheck the pin against the official release before packaging.`,
            );
    }
    return runtimeRoot;
}
