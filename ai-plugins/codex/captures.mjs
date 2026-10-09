// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore IHDR
import { watch } from "node:fs";
import { randomUUID } from "node:crypto";
import { mkdir, readFile, readdir, rename, stat, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const maxImageBytes = 4 * 1024 * 1024;
const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export function createCaptureStore(directory = join(tmpdir(), `slint-codex-captures-${process.getuid?.() ?? process.env.USERNAME ?? "user"}`)) {
  const waiting = new Map();
  let watcher;
  function watchChanges() {
    if (watcher) return;
    try {
      watcher = watch(directory, { persistent: false }, (_event, filename) => {
        if (filename?.endsWith(".json")) notify(filename.slice(0, -5));
        else if (!filename) for (const id of waiting.keys()) notify(id);
      });
      watcher.on("error", () => { watcher?.close(); watcher = undefined; for (const id of waiting.keys()) notify(id); });
    } catch (error) { if (error.code !== "ENOENT") throw error; }
  }
  function notify(previewId) { for (const callback of waiting.get(previewId) ?? []) callback(); }
  function path(id) {
    if (!/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(id)) throw new Error("Invalid Slint preview ID.");
    return join(directory, id + ".json");
  }
  async function read(id) {
    try { return JSON.parse(await readFile(path(id), "utf8")); }
    catch (error) { if (error.code === "ENOENT") return undefined; throw error; }
  }
  async function write(record) {
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const temporary = path(record.previewId) + "." + randomUUID() + ".tmp";
    try {
      await writeFile(temporary, JSON.stringify(record), { mode: 0o600 });
      await rename(temporary, path(record.previewId));
    } finally { await unlink(temporary).catch(error => { if (error.code !== "ENOENT") throw error; }); }
  }
  function check(record, revision, sourceHash) {
    if (record.revision !== revision || record.sourceHash !== sourceHash) throw new Error("The screenshot does not match the requested source revision.");
  }
  const outcome = record => ({ status: record.status, previewId: record.previewId, revision: record.revision, sourceHash: record.sourceHash, captureId: record.captureId, capturedAt: record.capturedAt, message: record.message });
  return {
    async create(identity) {
      const record = { ...identity, previewId: randomUUID(), captureToken: randomUUID(), captureId: randomUUID(), status: "pending" };
      await write(record);
      const entries = await Promise.all((await readdir(directory)).filter(name => name.endsWith(".json")).map(async name => {
        try { return { name, time: (await stat(join(directory, name))).mtimeMs }; }
        catch (error) { if (error.code === "ENOENT") return undefined; throw error; }
      }));
      for (const entry of entries.filter(Boolean).sort((a, b) => b.time - a.time).slice(32)) await unlink(join(directory, entry.name)).catch(error => { if (error.code !== "ENOENT") throw error; });
      return record;
    },
    async publish(args) {
      const record = await read(args.previewId);
      if (!record || record.captureToken !== args.captureToken) throw new Error("The Slint capture submission is unavailable or unauthorized.");
      check(record, args.revision, args.sourceHash);
      if (args.captureId !== record.captureId) throw new Error("This capture request was superseded.");
      if (record.status !== "pending") return outcome(record);
      if (typeof args.error === "string" && args.error.length && args.error.length <= 1024) {
        Object.assign(record, { status: "error", message: args.error });
      } else {
        if (typeof args.data !== "string" || args.data.length > Math.ceil(maxImageBytes / 3) * 4 || /[^A-Za-z0-9+/=]/.test(args.data)) throw new Error("Provide a PNG capture of at most 4 MiB.");
        const bytes = Buffer.from(args.data, "base64");
        if (bytes.length > maxImageBytes) throw new Error("Provide a PNG capture of at most 4 MiB.");
        if (bytes.toString("base64") !== args.data || bytes.length < 45 || !bytes.subarray(0, 8).equals(signature) || bytes.toString("ascii", 12, 16) !== "IHDR") throw new Error("The capture must be a PNG image.");
        const width = bytes.readUInt32BE(16), height = bytes.readUInt32BE(20);
        if (!width || !height || width > 4096 || height > 4096) throw new Error("Capture dimensions must be from 1 to 4096 pixels.");
        if (!Number.isSafeInteger(args.capturedAt) || args.capturedAt < 1) throw new Error("Provide the capture timestamp.");
        Object.assign(record, { status: "ready", data: args.data, width, height, capturedAt: args.capturedAt });
      }
      await write(record);
      return outcome(record);
    },
    async request(args) {
      const record = await read(args.previewId);
      if (!record) return { status: "unavailable", previewId: args.previewId, message: "This preview capture has expired. Submit the saved source again." };
      check(record, args.revision, args.sourceHash);
      record.captureId = randomUUID();
      record.status = "pending";
      for (const field of ["data", "width", "height", "capturedAt", "message"]) delete record[field];
      await write(record);
      notify(args.previewId);
      return outcome(record);
    },
    wait(previewId, afterCaptureId, timeoutMs = 20000) {
      return new Promise((resolve, reject) => {
        watchChanges();
        const callbacks = waiting.get(previewId) ?? new Set();
        waiting.set(previewId, callbacks);
        let finished = false;
        const complete = (result, error) => {
          if (finished) return;
          finished = true;
          clearTimeout(timer);
          callbacks.delete(changed);
          if (!callbacks.size) waiting.delete(previewId);
          if (error) reject(error); else resolve(result);
        };
        const changed = async (closed = false) => {
          if (closed) { complete({ status: "unavailable", previewId }); return; }
          try {
            const record = await read(previewId);
            if (!record) complete({ status: "unavailable", previewId });
            else if (record.captureId !== afterCaptureId) complete(outcome(record));
          } catch (error) { complete(undefined, error); }
        };
        const timer = setTimeout(() => complete({ status: "idle", previewId, captureId: afterCaptureId }), timeoutMs);
        callbacks.add(changed);
        void changed();
      });
    },
    close() {
      watcher?.close(); watcher = undefined;
      for (const callbacks of waiting.values()) for (const callback of callbacks) callback(true);
    },
    async get({ previewId, revision, sourceHash, captureId }) {
      const record = await read(previewId);
      if (!record) return { status: "unavailable", previewId, message: "This preview capture has expired. Submit the saved source again." };
      check(record, revision, sourceHash);
      if (captureId !== undefined && captureId !== record.captureId) throw new Error("This capture request was superseded.");
      const { captureToken, ...result } = record;
      return result;
    },
  };
}
