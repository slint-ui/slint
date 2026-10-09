// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { randomUUID } from "node:crypto";
import { watch } from "node:fs";
import { mkdir, readFile, readdir, rename, stat, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const empty = "00000000-0000-0000-0000-000000000000";
export function createViewStore(directory = join(tmpdir(), `slint-codex-views-${process.getuid?.() ?? process.env.USERNAME ?? "user"}`)) {
  const waiting = new Set();
  function path(id) {
    if (!/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(id)) throw new Error("Invalid Slint view ID.");
    return join(directory, id + ".json");
  }
  async function read(id) {
    try { return JSON.parse(await readFile(path(id), "utf8")); }
    catch (error) { if (error.code === "ENOENT") throw new Error("This Slint view has expired. Open the side preview again."); throw error; }
  }
  async function write(id, result) {
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const temporary = path(id) + "." + randomUUID() + ".tmp";
    try { await writeFile(temporary, JSON.stringify(result), { mode: 0o600 }); await rename(temporary, path(id)); }
    finally { await unlink(temporary).catch(error => { if (error.code !== "ENOENT") throw error; }); }
  }
  return {
    async create(result) {
      const id = randomUUID();
      result.structuredContent.viewId = id;
      await write(id, result);
      const entries = await Promise.all((await readdir(directory)).filter(name => name.endsWith(".json")).map(async name => {
        try { return { name, time: (await stat(join(directory, name))).mtimeMs }; }
        catch (error) { if (error.code !== "ENOENT") throw error; return undefined; }
      }));
      for (const entry of entries.filter(Boolean).sort((a, b) => b.time - a.time).slice(32)) await unlink(join(directory, entry.name)).catch(error => { if (error.code !== "ENOENT") throw error; });
      return result;
    },
    read,
    async update(id, result) {
      const previous = await read(id);
      if (previous.structuredContent.sourcePath && previous.structuredContent.sourcePath !== result.structuredContent.sourcePath) throw new Error("Update the same source in this Slint view.");
      if (previous.structuredContent.revision >= result.structuredContent.revision) throw new Error("Increment the source revision when updating a Slint view.");
      result.structuredContent.viewId = id;
      await write(id, result);
      return result;
    },
    wait(id, after, timeoutMs = 20000) {
      return new Promise((resolve, reject) => {
        let finished = false;
        const complete = (result, error) => {
          if (finished) return;
          finished = true; clearTimeout(timer); watcher.close(); waiting.delete(close);
          if (error) reject(error); else resolve(result);
        };
        const changed = async () => {
          try { const result = await read(id); if ((result.structuredContent.previewId ?? empty) !== after) complete(result); }
          catch (error) { complete(undefined, error); }
        };
        const close = () => complete({ status: "closed" });
        const watcher = watch(directory, { persistent: false }, (_event, filename) => { if (!filename || filename === id + ".json") void changed(); });
        watcher.on("error", error => complete(undefined, error));
        const timer = setTimeout(() => complete({ status: "idle" }), timeoutMs);
        waiting.add(close); void changed();
      });
    },
    close() { for (const close of waiting) close(); },
  };
}
