// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import JSZip from "jszip";

const bytes = await readFile("zip/figma-plugin.zip");
const archive = await JSZip.loadAsync(bytes);
const { version } = JSON.parse(await readFile("package.json", "utf8"));
const prefix = `Figma to Slint_${version}/`;
const provenance = JSON.parse(
    await archive.file(`${prefix}provenance.json`).async("string"),
);
const manifest = JSON.parse(
    await archive.file(`${prefix}manifest.json`).async("string"),
);
assert.equal(provenance.channel, "nightly");
assert.equal(provenance.repository, "https://github.com/slint-ui/slint.git");
assert.equal(provenance.manifest, "api/wasm-interpreter/Cargo.toml");
assert.equal(provenance.dirty, false);
assert.equal(manifest.id, "1474418299182276871");
assert.deepEqual(manifest.networkAccess.allowedDomains, ["none"]);
assert.ok(
    (await archive.file("readme.txt").async("string")).includes(
        "not a Community release",
    ),
);
for (const field of ["cargoTargetDir", "branch", "materializedAt", "cacheHit"])
    assert.equal(field in provenance, false);
const sums = JSON.parse(
    await archive.file(`${prefix}SHA256SUMS.json`).async("string"),
);
const files = Object.values(archive.files)
    .filter((entry) => !entry.dir)
    .map((entry) => entry.name);
assert.deepEqual(
    Object.keys(sums).sort(),
    files.filter((name) => name !== `${prefix}SHA256SUMS.json`).sort(),
);
for (const [name, hash] of Object.entries(sums))
    assert.equal(
        createHash("sha256")
            .update(await archive.file(name).async("nodebuffer"))
            .digest("hex"),
        hash,
    );
function rejects(args, pattern, env = process.env) {
    assert.throws(
        () =>
            execFileSync(
                process.execPath,
                ["scripts/package-plugin.mjs", ...args],
                { env, stdio: "pipe" },
            ),
        (error) => pattern.test(String(error.stderr)),
    );
}
rejects(["--nightly", "--test"], /Choose exactly one packaging mode/);
rejects(["--release-please"], /Choose exactly one packaging mode/);
rejects(["--nightly"], /FIGMA_PLUGIN_ID must be an assigned/, {
    ...process.env,
    FIGMA_PLUGIN_ID: "000000000000000000",
});
const pin = JSON.parse(await readFile("runtime-pin.json", "utf8"));
if (pin.development !== false)
    rejects([], /Development snapshots cannot be published/);
assert.deepEqual(await readFile("zip/figma-plugin.zip"), bytes);
console.log(
    "Validated nightly identity, portable provenance, ZIP checksums and packaging-mode isolation",
);
