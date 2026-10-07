// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { test } from "node:test";
import { openRuntimeCache } from "../preview/runtime-cache.mjs";

function storage(records) {
  return { open() {
    const request = {};
    queueMicrotask(() => {
      request.result = {
        close() {},
        transaction() {
          const transaction = {
            objectStore() { return {
              get(key) { const request = { result: structuredClone(records.get(key)) }; queueMicrotask(() => transaction.oncomplete()); return request; },
              put(value, key) { records.set(key, structuredClone(value)); const request = {}; queueMicrotask(() => transaction.oncomplete()); return request; },
            }; },
          };
          return transaction;
        },
      };
      request.onsuccess();
    });
    return request;
  } };
}

test("runtime cache keeps only the latest runtime and survives a new connection", async () => {
  const original = globalThis.indexedDB;
  const records = new Map();
  globalThis.indexedDB = storage(records);
  try {
    const first = await openRuntimeCache();
    await first.write({ key: "one", javascript: "first", wasm: new Uint8Array([1, 2]).buffer });
    first.close();
    const reopened = await openRuntimeCache();
    assert.deepEqual(new Uint8Array((await reopened.read("one")).wasm), new Uint8Array([1, 2]));
    assert.equal(await reopened.read("changed-runtime"), undefined);
    await reopened.write({ key: "two", javascript: "second", wasm: new ArrayBuffer(3) });
    assert.equal(records.size, 1);
    assert.equal(await reopened.read("one"), undefined);
    assert.equal((await reopened.read("two")).javascript, "second");
    records.set("current", { key: "two", javascript: "second", wasm: "invalid" });
    assert.equal(await reopened.read("two"), undefined);
    reopened.close();
  } finally { globalThis.indexedDB = original; }
});

test("storage restrictions leave the normal resource loading path available", async () => {
  const original = globalThis.indexedDB;
  globalThis.indexedDB = { open() { throw new Error("Storage denied"); } };
  try { assert.equal(await openRuntimeCache(), undefined); }
  finally { globalThis.indexedDB = original; }
});
