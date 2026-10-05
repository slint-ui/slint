// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { execFileSync } from "node:child_process";

function buildCommit() {
    try {
        return execFileSync("git", ["rev-parse", "HEAD"], {
            encoding: "utf8",
            stdio: ["ignore", "pipe", "ignore"],
        }).trim();
    } catch {
        const hash = process.env.GITHUB_SHA;
        if (hash) {
            return hash;
        }
        throw new Error(
            "Can't determine the commit the safety manual is built from: run the build in a git checkout, or set GITHUB_SHA",
        );
    }
}

/** Full hash of the commit the safety manual is built from. */
export const BUILD_COMMIT = buildCommit();
