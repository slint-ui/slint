// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { mkdtemp, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { createCaptureStore } from "../captures.mjs";

const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9BkAAAAASUVORK5CYII=";
const identity = { revision: 1, sourceHash: "a".repeat(64), projectHash: "b".repeat(64), runtimeRevision: "runtime" };

test("captures preserve source identity across connections and reject mismatches", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-capture-test-"));
  try {
    const store = createCaptureStore(directory);
    const record = await store.create(identity);
    const query = { previewId: record.previewId, revision: record.revision, sourceHash: record.sourceHash };
    assert.equal((await store.get(query)).status, "pending");
    assert.equal((await store.get(query)).captureToken, undefined);
    const submission = { ...query, captureToken: record.captureToken, captureId: record.captureId, data: png, capturedAt: Date.now() };
    await assert.rejects(store.publish({ ...submission, captureToken: "wrong" }), /unauthorized/);
    await assert.rejects(store.publish({ ...submission, revision: 2 }), /does not match/);
    await assert.rejects(store.publish({ ...submission, sourceHash: "c".repeat(64) }), /does not match/);
    assert.equal((await store.publish(submission)).status, "ready");
    const result = await createCaptureStore(directory).get(query);
    assert.equal(result.data, png);
    assert.equal(result.capturedAt, submission.capturedAt);
    assert.equal(result.width, 1);
    assert.equal(result.height, 1);
    assert.equal(result.captureToken, undefined);
    await assert.rejects(store.get({ ...query, revision: 2 }), /does not match/);
    const retry = await store.publish({ ...submission, capturedAt: submission.capturedAt + 1 });
    assert.equal(retry.status, "ready");
    assert.equal(retry.capturedAt, submission.capturedAt);
    assert.equal((await store.get(query)).data, png);
    if (process.platform !== "win32") assert.equal((await stat(join(directory, record.previewId + ".json"))).mode & 0o777, 0o600);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("invalid captures remain pending and capture failures are explicit", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-capture-invalid-"));
  try {
    const store = createCaptureStore(directory), record = await store.create(identity);
    const args = { previewId: record.previewId, captureToken: record.captureToken, captureId: record.captureId, revision: 1, sourceHash: identity.sourceHash, capturedAt: Date.now() };
    for (const data of ["bad!", "AAAA", "A".repeat(5592412)]) await assert.rejects(store.publish({ ...args, data }), /PNG/);
    const oversized = Buffer.from(png, "base64");
    oversized.writeUInt32BE(4097, 16);
    await assert.rejects(store.publish({ ...args, data: oversized.toString("base64") }), /dimensions/);
    await assert.rejects(store.publish({ ...args, data: png, capturedAt: 0 }), /timestamp/);
    assert.equal((await store.get(args)).status, "pending");
    assert.equal((await store.publish({ ...args, error: "Capture is unsupported." })).status, "error");
    assert.equal((await store.get(args)).message, "Capture is unsupported.");
    await assert.rejects(store.get({ ...args, previewId: "../outside" }), /Invalid Slint preview ID/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("capture retention is bounded and expired captures are unavailable", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-capture-retention-"));
  try {
    const store = createCaptureStore(directory);
    for (let i = 0; i < 34; i++) await store.create({ ...identity, revision: i + 1 });
    assert.equal((await readdir(directory)).length, 32);
    const missing = await store.get({ previewId: "00000000-0000-0000-0000-000000000000", revision: 1, sourceHash: identity.sourceHash });
    assert.equal(missing.status, "unavailable");
    assert.equal(missing.data, undefined);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("fresh captures preserve source identity and reject superseded images", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-capture-fresh-"));
  try {
    const store = createCaptureStore(directory), record = await store.create(identity);
    const query = { previewId: record.previewId, revision: 1, sourceHash: identity.sourceHash };
    const first = { ...query, captureToken: record.captureToken, captureId: record.captureId, data: png, capturedAt: 1 };
    await store.publish(first);
    const priorPending = await store.request(query);
    const waiting = store.wait(record.previewId, priorPending.captureId, 1000);
    const requester = createCaptureStore(directory);
    const fresh = await requester.request(query);
    assert.equal(fresh.status, "pending");
    assert.notEqual(fresh.captureId, record.captureId);
    assert.notEqual(fresh.captureId, priorPending.captureId);
    assert.equal((await waiting).captureId, fresh.captureId);
    assert.equal((await store.get({ ...query, captureId: fresh.captureId })).data, undefined);

    await assert.rejects(store.get({ ...query, captureId: record.captureId }), /superseded/);
    await assert.rejects(store.publish(first), /superseded/);
    await store.publish({ ...first, captureId: fresh.captureId, capturedAt: 2 });
    const image = await createCaptureStore(directory).get({ ...query, captureId: fresh.captureId });
    assert.equal(image.capturedAt, 2);
    assert.equal(image.revision, 1);
    assert.equal(image.sourceHash, identity.sourceHash);
    assert.equal(image.data, png);
    assert.equal((await store.wait(record.previewId, fresh.captureId, 1)).status, "idle");
    const closing = store.wait(record.previewId, fresh.captureId);
    store.close();
    assert.equal((await closing).status, "unavailable");
  } finally { await rm(directory, { recursive: true, force: true }); }
});
