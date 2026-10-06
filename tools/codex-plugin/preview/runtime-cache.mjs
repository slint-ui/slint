export async function openRuntimeCache() {
  let database;
  try {
    database = await new Promise((resolve, reject) => {
      const request = indexedDB.open("slint-preview-runtime", 1);
      let expired = false;
      const timeout = setTimeout(() => { expired = true; reject(new Error("Runtime cache unavailable")); }, 500);
      request.onupgradeneeded = () => request.result.createObjectStore("runtime");
      request.onsuccess = () => { clearTimeout(timeout); if (expired) request.result.close(); else resolve(request.result); };
      request.onerror = () => { clearTimeout(timeout); reject(request.error); };
      request.onblocked = () => { expired = true; clearTimeout(timeout); reject(new Error("Runtime cache blocked")); };
    });
  } catch { return undefined; }
  const operation = (mode, run) => new Promise((resolve, reject) => {
    const transaction = database.transaction("runtime", mode);
    const request = run(transaction.objectStore("runtime"));
    const timeout = setTimeout(() => { transaction.abort(); reject(new Error("Runtime cache timed out")); }, 500);
    transaction.oncomplete = () => { clearTimeout(timeout); resolve(request.result); };
    transaction.onabort = transaction.onerror = () => { clearTimeout(timeout); reject(transaction.error); };
  });
  return {
    async read(key) {
      try {
        const value = await operation("readonly", store => store.get("current"));
        return value?.key === key && value.wasm instanceof ArrayBuffer && typeof value.javascript === "string" ? value : undefined;
      } catch { return undefined; }
    },
    async write(value) {
      try { await operation("readwrite", store => store.put(value, "current")); } catch {}
    },
    close() { database.close(); },
  };
}
