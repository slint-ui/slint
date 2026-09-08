// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runtimePin, runtimeRoot, verifyRuntime } from "./runtime-pin.mjs";
import { validateSlintArtifact } from "./slint-artifact.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputDir = resolve(projectRoot, ".generated/slint-wasm");
export async function verifyArtifact(directory, release = false) {
    verifyRuntime(release);
    const sourceTree = execFileSync(
        "git",
        ["-C", runtimeRoot, "rev-parse", "HEAD^{tree}"],
        { encoding: "utf8" },
    ).trim();
    return validateSlintArtifact(directory, {
        runtimePin,
        revision: runtimePin.revision,
        sourceTree,
    });
}
if (
    process.argv[1] &&
    resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
    const provenance = await verifyArtifact(outputDir);
    console.log(
        `Validated Slint interpreter artifact at revision ${provenance.revision}`,
    );
}
