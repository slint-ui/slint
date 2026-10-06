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
