// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { loadRuntime, publishRuntime } from "../runtime-assets.mjs";

async function fixture(root, revision) {
  await mkdir(join(root, "wasm"), { recursive: true });
  await writeFile(join(root, "runtime.json"), JSON.stringify({ version: "1.19.0", revision, platform: process.platform, architecture: process.arch }));
  const executable = join(root, process.platform === "win32" ? "slint-lsp.exe" : "slint-lsp");
  await writeFile(executable, "test executable");
  await chmod(executable, 0o755);
  await writeFile(join(root, "wasm/slint_wasm_interpreter_bg.wasm"), Buffer.from([0,97,115,109,1,0,0,0]));
  await writeFile(join(root, "preview.html"), "<!doctype html>");
  await writeFile(join(root, "slint.svg"), "<svg/>");
}

test("incomplete builds do not replace a working runtime", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-runtime-ready-"));
  const destination = join(directory, "runtime");
  const staging = join(directory, "staging");
  try {
    await fixture(destination, "old");
    await fixture(staging, "new");
    await rm(join(staging, "preview.html"));
    assert.equal(await loadRuntime(staging), null);
    await assert.rejects(publishRuntime(staging, destination), /incomplete/);
    assert.equal((await loadRuntime(destination)).metadata.revision, "old");
    await writeFile(join(staging, "preview.html"), "<!doctype html>");
    await publishRuntime(staging, destination);
    assert.equal((await loadRuntime(destination)).metadata.revision, "new");
    assert.equal(await loadRuntime(staging), null);
    await writeFile(join(destination, "runtime.json"), "{");
    assert.equal(await loadRuntime(destination), null);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
