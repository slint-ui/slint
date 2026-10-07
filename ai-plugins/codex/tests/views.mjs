// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { createViewStore } from "../views.mjs";

const result = (revision, previewId) => ({ structuredContent: { revision, previewId, sourcePath: "/project/main.slint", status: "submitted" }, _meta: { preview: { source: "source", project: {} } } });
test("a persistent view receives updates across processes without changing its ID", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-view-test-"));
  const reader = createViewStore(directory), writer = createViewStore(directory);
  try {
    const initial = await writer.create(result(1, "first"));
    const id = initial.structuredContent.viewId;
    const waiting = reader.wait(id, "first", 1000);
    const updated = await writer.update(id, result(2, "second"));
    assert.equal(updated.structuredContent.viewId, id);
    assert.equal((await waiting).structuredContent.previewId, "second");
    assert.equal((await createViewStore(directory).read(id))._meta.preview.source, "source");
    await assert.rejects(writer.update(id, result(2, "duplicate")), /Increment/);
    const other = result(3, "other"); other.structuredContent.sourcePath = "/project/other.slint";
    await assert.rejects(writer.update(id, other), /same source/);
    const closing = reader.wait(id, "second"); reader.close();
    assert.equal((await closing).status, "closed");
  } finally { reader.close(); writer.close(); await rm(directory, { recursive: true, force: true }); }
});
