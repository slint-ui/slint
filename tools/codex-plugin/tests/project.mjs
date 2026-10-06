// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { readProjectResource, snapshotProject } from "../project.mjs";

test("nested sources and assets survive source changes in an immutable snapshot", async () => {
  const root = await mkdtemp(join(tmpdir(), "slint-project-test-"));
  try {
    await mkdir(join(root, "ui"));
    await mkdir(join(root, "assets"));
    await writeFile(join(root, "main.slint"), 'import { Card } from "ui/card.slint"; export component Preview inherits Window { Card {} }');
    const component = 'import "Inter.ttf"; export component Card inherits Rectangle { Image { source: @image-url("../assets/pixel.svg"); } Text { text: "import \\\"missing.slint\\\";"; font-family: "Inter"; } }';
    await writeFile(join(root, "ui/card.slint"), component);
    await copyFile(new URL("../../../internal/common/sharedfontique/Inter-VariableFont.ttf", import.meta.url), join(root, "ui/Inter.ttf"));
    await writeFile(join(root, "assets/pixel.svg"), '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="blue"/></svg>');
    const snapshot = await snapshotProject(join(root, "main.slint"), root, "");
    assert.deepEqual(Object.keys(snapshot.files).sort(), ["assets/pixel.svg", "main.slint", "ui/Inter.ttf", "ui/card.slint"]);
    const uri = snapshot.files["ui/card.slint"].uris[0];
    await writeFile(join(root, "ui/card.slint"), component + "\n");
    assert.equal(Buffer.from((await readProjectResource(uri)).blob, "base64").toString(), component);
    const changed = await snapshotProject(join(root, "main.slint"), root, "");
    assert.notEqual(changed.id, snapshot.id);
    await assert.rejects(readProjectResource(uri.replace("/0", "/99999")), /Unknown project resource chunk/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("dependencies outside the declared root, including symlinks, are rejected", async () => {
  const root = await mkdtemp(join(tmpdir(), "slint-project-boundary-"));
  try {
    await mkdir(join(root, "project"));
    await writeFile(join(root, "outside.slint"), "export component Outside inherits Rectangle {}");
    const path = join(root, "project/main.slint");
    await writeFile(path, 'import { Outside } from "../outside.slint"; export component Preview inherits Window { Outside {} }');
    await assert.rejects(snapshotProject(path, join(root, "project"), ""), /outside the declared project root/);
    await symlink(join(root, "outside.slint"), join(root, "project/link.slint"));
    await writeFile(path, 'import { Outside } from "link.slint"; export component Preview inherits Window { Outside {} }');
    await assert.rejects(snapshotProject(path, join(root, "project"), ""), /outside the declared project root/);
  } finally { await rm(root, { recursive: true, force: true }); }
});


test("re-exported components include their assets and preserve empty images", async () => {
  const root = await mkdtemp(join(tmpdir(), "slint-re-export-test-"));
  try {
    await mkdir(join(root, "ui"));
    await writeFile(join(root, "main.slint"), 'import { Card } from "ui/pages.slint"; export component Preview inherits Window { Card {} }');
    await writeFile(join(root, "ui/pages.slint"), 'export { Card } from "card.slint";');
    const card = 'export component Card inherits Rectangle { Image { source: @image-url(""); } Image { source: @image-url("check.svg"); } }';
    await writeFile(join(root, "ui/card.slint"), card);
    await writeFile(join(root, "ui/check.svg"), '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>');
    const first = await snapshotProject(join(root, "main.slint"), root, "");
    assert.deepEqual(Object.keys(first.files).sort(), ["main.slint", "ui/card.slint", "ui/check.svg", "ui/pages.slint"]);
    assert.equal(Buffer.from((await readProjectResource(first.files["ui/card.slint"].uris[0])).blob, "base64").toString(), card);
    await writeFile(join(root, "ui/card.slint"), card.replace('inherits Rectangle {', 'inherits Rectangle { background: blue;'));
    const second = await snapshotProject(join(root, "main.slint"), root, "");
    assert.equal(first.files["main.slint"].hash, second.files["main.slint"].hash);
    assert.notEqual(first.id, second.id);
    assert.notEqual(first.files["ui/card.slint"].hash, second.files["ui/card.slint"].hash);
    await writeFile(join(root, "ui/pages.slint"), 'export { Card } from "../../outside.slint";');
    await assert.rejects(snapshotProject(join(root, "main.slint"), root, ""), /outside the declared project root/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
