import { highlight } from "./syntax.mjs";
import { installPreviewZoom } from "./zoom.mjs";
import { openRuntimeCache } from "./runtime-cache.mjs";

const buildInfo = JSON.parse(document.getElementById("slint-build").textContent);
const runtimeInfo = JSON.parse(document.getElementById("slint-runtime").textContent);
const buttonSource = JSON.parse(document.getElementById("slint-button").textContent);
const version = document.getElementById("version");
version.textContent = `v${buildInfo.version}`;
const status = document.getElementById("status");
const canvas = document.getElementById("slint-preview");
document.addEventListener("keydown", event => {
  if (event.key === "Tab") canvas.dataset.keyboardFocus = "true";
}, true);
document.addEventListener("pointerdown", () => {
  canvas.dataset.keyboardFocus = "false";
}, true);
const example = JSON.parse(document.getElementById("slint-example").textContent);
const loadingStarted = performance.now();
const loadingTimings = {};
function markLoading(phase) {
  loadingTimings[phase] = Math.round(performance.now() - loadingStarted);
  console.info("Slint preview timing", JSON.stringify({ phase, elapsedMs: loadingTimings[phase], revision: desired.revision, runtimeHash: runtimeInfo.wasmHash, documentStartedAt: performance.timeOrigin, measuredAt: Date.now() }));
}
let desired = { source: example, revision: 0, width: 320, height: 160 };
markLoading("document-ready");
let generation = 0;
let appliedGeneration = -1;
let rendering = false;
let runtime;
let instance;
let zoom;
const codePanel = document.getElementById("code-panel");
const previewPanel = document.getElementById("preview-panel");
const menuToggle = document.getElementById("view-menu-toggle");
const menu = document.getElementById("view-menu");
const views = [...menu.querySelectorAll("button")];
let hostTheme;
const colorScheme = matchMedia("(prefers-color-scheme: dark)");
let highlightRequest = 0;
function updateCode() {
  const source = desired.source;
  const revision = desired.revision;
  const request = ++highlightRequest;
  const pre = document.createElement("pre");
  const code = document.createElement("code");
  code.textContent = source;
  pre.appendChild(code);
  codePanel.replaceChildren(pre);
  codePanel.dataset.revision = String(revision);
  codePanel.dataset.highlight = "plain";
  if (codePanel.hidden) return;
  void highlight(source, (hostTheme ?? (colorScheme.matches ? "dark" : "light")) === "dark" ? "dark-slint" : "light-slint").then(html => {
    if (request !== highlightRequest) return;
    codePanel.innerHTML = html;
    codePanel.dataset.highlight = "ready";
  }).catch(error => console.error("Syntax highlighting unavailable", error));
}
function closeMenu(returnFocus = false) {
  menu.hidden = true;
  menuToggle.setAttribute("aria-expanded", "false");
  if (returnFocus) menuToggle.focus();
}
function openMenu(index = views.findIndex(item => item.getAttribute("aria-checked") === "true")) {
  menu.hidden = false;
  menuToggle.setAttribute("aria-expanded", "true");
  views[index].focus();
}
menuToggle.addEventListener("click", () => menu.hidden ? openMenu() : closeMenu());
menuToggle.addEventListener("keydown", event => {
  if (!["ArrowDown", "ArrowUp"].includes(event.key)) return;
  event.preventDefault();
  openMenu(event.key === "ArrowDown" ? 0 : views.length - 1);
});
for (const view of views) view.addEventListener("click", () => {
  for (const item of views) item.setAttribute("aria-checked", String(item === view));
  previewPanel.hidden = view.dataset.view !== "preview";
  codePanel.hidden = view.dataset.view !== "code";
  if (codePanel.hidden) zoom?.refresh(); else updateCode();
  closeMenu(true);
});
menu.addEventListener("keydown", event => {
  if (event.key === "Escape") { event.preventDefault(); closeMenu(true); return; }
  const index = views.indexOf(document.activeElement);
  const next = event.key === "ArrowDown" ? (index + 1) % views.length : event.key === "ArrowUp" ? (index + views.length - 1) % views.length : event.key === "Home" ? 0 : event.key === "End" ? views.length - 1 : undefined;
  if (next !== undefined) { event.preventDefault(); views[next].focus(); }
});
document.addEventListener("pointerdown", event => {
  if (!menu.contains(event.target) && !menuToggle.contains(event.target)) closeMenu();
});
document.addEventListener("focusin", event => {
  if (!menu.contains(event.target) && !menuToggle.contains(event.target)) closeMenu();
});
function applyHostContext(context) {
  if (context?.theme === "light" || context?.theme === "dark") {
    hostTheme = context.theme;
    menu.style.colorScheme = hostTheme;
    codePanel.style.colorScheme = hostTheme;
    document.documentElement.style.setProperty("--fallback-text", hostTheme === "dark" ? "#ededed" : "#15191e");
    document.documentElement.style.setProperty("--fallback-muted", hostTheme === "dark" ? "#ababab" : "#626973");
  }
  for (const [name, value] of Object.entries(context?.styles?.variables ?? {})) {
    if (name.startsWith("--") && typeof value === "string") document.documentElement.style.setProperty(name, value);
  }
  updateCode();
}
colorScheme.addEventListener("change", updateCode);
zoom = installPreviewZoom({ canvas, initialSize: desired });
updateCode();
new ResizeObserver(() => {
  if (window.parent === window) return;
  window.parent.postMessage({ jsonrpc: "2.0", method: "ui/notifications/size-changed", params: { height: Math.ceil(document.body.getBoundingClientRect().height) } }, "*");
}).observe(document.body);

function report(state, revision, diagnostics, sourceHash) {
  if (window.parent === window) return;
  window.parent.postMessage({
    jsonrpc: "2.0", id: "slint-context-" + revision + "-" + state,
    method: "ui/update-model-context",
    params: { structuredContent: { slintPreview: { previewVersion: buildInfo.version, buildId: buildInfo.buildId, runtimeVersion: runtimeInfo.version, runtimeRevision: runtimeInfo.revision, sourcePath: desired.sourcePath, projectRoot: desired.projectRoot, revision, sourceHash, state, diagnostics, loadingTimings } } },
  }, "*");
}
function showError(error, acknowledge = true) {
  status.hidden = false;
  status.textContent = String(error);
  document.documentElement.dataset.slint = "error";
  canvas.dataset.stale = String(Boolean(instance));
  console.error(error);
  if (acknowledge) report("error", desired.revision, [{ level: "error", message: String(error) }], desired.sourceHash);
}
function setSource(input) {
  if (!input || typeof input.source !== "string" || !input.source.length || input.source.length > 65536) return;
  if (!Number.isSafeInteger(input.revision) || input.revision < desired.revision) return;
  const next = { width: 320, height: 160, ...input };
  if (!Number.isSafeInteger(next.width) || next.width < 64 || next.width > 2048 ||
      !Number.isSafeInteger(next.height) || next.height < 64 || next.height > 2048) return;
  if (next.revision === desired.revision) {
    if (next.source !== desired.source) return;
    if (next.width === desired.width && next.height === desired.height) {
      desired = { ...desired, ...next };
      return;
    }
  }
  desired = next;
  markLoading("source-received");
  updateCode();
  generation += 1;
  void render().catch(showError);
}
async function render() {
  if (!runtime || rendering) return;
  rendering = true;
  try {
    while (appliedGeneration !== generation) {
      const current = { ...desired };
      const token = generation;
      const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(current.source));
      current.sourceHash = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
      if (current.source === desired.source) desired.sourceHash = current.sourceHash;
      markLoading("project-start");
      const project = await prepareProject(current.project, current.source, current.sourceHash);
      markLoading("project-ready");
      const result = await runtime.compile_from_string(current.source, project.baseUrl, async path => {
        const key = decodeURIComponent(new URL(path, project.baseUrl).pathname).replace(project.prefix, "");
        if (project.sources.has(key)) return project.sources.get(key);
        if (path === "slint-button.slint" || path.endsWith("/slint-button.slint")) return buttonSource;
        throw new Error("Unknown bundled import: " + path);
      }, project.images);
      markLoading("source-compiled");
      const component = result.component;
      const projectPrefix = current.project ? "file://" + project.prefix : undefined;
      const diagnostics = result.diagnostics.map(diagnostic => ({ ...diagnostic,
        fileName: projectPrefix && diagnostic.fileName.startsWith(projectPrefix)
          ? current.projectRoot + "/" + decodeURIComponent(diagnostic.fileName.slice(projectPrefix.length)) : diagnostic.fileName,
      }));
      if (token !== generation) { component?.free(); result.free(); continue; }
      if (!component || result.error_string.trim()) {
        const errorText = projectPrefix ? result.error_string.split(projectPrefix).join(current.projectRoot + "/") : result.error_string;
        showError((instance ? "Preview paused — showing the last valid revision.\n" : "Couldn’t preview.\n") + errorText, false);
        report("error", current.revision, diagnostics, current.sourceHash);
        component?.free();
        result.free();
        appliedGeneration = token;
        continue;
      }
      canvas.style.width = current.width + "px";
      canvas.style.height = current.height + "px";
      if (instance) {
        const previous = instance;
        instance = undefined;
        instance = await component.create_with_existing_window(previous);
      } else {
        canvas.hidden = false;
        const instancePromise = component.create(canvas.id);
        try { runtime.run_event_loop(); } catch (error) {
          if (!(typeof error === "string" && error.includes("control flow"))) throw error;
        }
        instance = await instancePromise;
      }
      markLoading("window-created");
      await instance.show();
      markLoading("window-shown");
      component.free();
      result.free();
      appliedGeneration = token;
      if (token !== generation) continue;
      canvas.dataset.stale = "false";
      canvas.dataset.revision = String(current.revision);
      status.hidden = true;
      zoom.setSize(current.width, current.height);
      document.documentElement.dataset.slint = "ready";
      document.documentElement.dataset.revision = String(current.revision);
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      if (token !== generation) continue;
      markLoading("paint-ready");
      report("ready", current.revision, diagnostics, current.sourceHash);
    }
  } finally { rendering = false; }
}
let resolveBridge;
let rejectBridge;
const bridgeReady = new Promise((resolve, reject) => { resolveBridge = resolve; rejectBridge = reject; });
const resourceRequests = new Map();
let resourceRequestId = 0;
function readRuntimeResource(uri) {
  return new Promise((resolve, reject) => {
    const id = `slint-runtime-${++resourceRequestId}`;
    const timeout = setTimeout(() => { resourceRequests.delete(id); reject(new Error("The host did not return a Slint runtime resource.")); }, 30000);
    resourceRequests.set(id, { resolve, reject, timeout });
    window.parent.postMessage({ jsonrpc: "2.0", id, method: "resources/read", params: { uri } }, "*");
  });
}
async function readBytes(uris) {
  const chunks = [];
  for (let offset = 0; offset < uris.length; offset += 4) {
    chunks.push(...await Promise.all(uris.slice(offset, offset + 4).map(async uri => {
      const resource = (await readRuntimeResource(uri)).contents[0];
      return Uint8Array.from(atob(resource.blob), character => character.charCodeAt(0));
    })));
  }
  return new Blob(chunks);
}
const registeredFonts = new Set();
let preparedProject;
function prepareProject(project, source, sourceHash) {
  if (!project) return Promise.resolve({ baseUrl: "file:///preview.slint", prefix: "/", sources: new Map(), images: undefined });
  if (preparedProject?.id === project.id) return preparedProject.promise;
  const prefix = `/__slint_preview/${project.id}/`;
  const baseUrl = "file://" + prefix + project.entry.split("/").map(encodeURIComponent).join("/");
  const promise = (async () => {
    const sources = new Map();
    const images = new Map();
    for (const [name, file] of Object.entries(project.files)) {
      const bytes = name === project.entry && file.hash === sourceHash
        ? new TextEncoder().encode(source) : new Uint8Array(await (await readBytes(file.uris)).arrayBuffer());
      const hash = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), byte => byte.toString(16).padStart(2, "0")).join("");
      if (hash !== file.hash) throw new Error("A project dependency does not match this preview.");
      if (file.mimeType === "text/plain") sources.set(name, new TextDecoder().decode(bytes));
      else if (file.mimeType.startsWith("font/")) {
        if (!registeredFonts.has(hash)) { runtime.register_font_from_memory(bytes); registeredFonts.add(hash); }
      } else {
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
        const imageUrl = `data:${file.mimeType};base64,${btoa(binary)}`;
        const image = new Image();
        image.src = imageUrl;
        try { await image.decode(); } catch { throw new Error("Cannot decode project image: " + name); }
        images.set("file://" + prefix + name.split("/").map(encodeURIComponent).join("/"), imageUrl);
      }
    }
    return { baseUrl, prefix, sources, images };
  })();
  preparedProject = { id: project.id, promise };
  return promise;
}
window.addEventListener("message", (event) => {
  if (event.source !== window.parent || event.data?.jsonrpc !== "2.0") return;
  const message = event.data;
  if (message.id === "slint-init" && message.result) {
    window.parent.postMessage({ jsonrpc: "2.0", method: "ui/notifications/initialized" }, "*");
    applyHostContext(message.result.hostContext);
    resolveBridge();
  }
  if (message.id === "slint-init" && message.error) rejectBridge(new Error(message.error.message));
  const request = resourceRequests.get(message.id);
  if (request) {
    clearTimeout(request.timeout);
    resourceRequests.delete(message.id);
    if (message.error) request.reject(new Error(message.error.message));
    else request.resolve(message.result);
  }
  if (message.method === "ui/notifications/host-context-changed") applyHostContext(message.params);
  if (message.method === "ui/notifications/tool-input") setSource(message.params?.arguments);
  if (message.method === "ui/notifications/tool-result") setSource(message.params?.structuredContent);
});
if (window.parent !== window) {
  window.parent.postMessage({
    jsonrpc: "2.0", id: "slint-init", method: "ui/initialize",
    params: { protocolVersion: "2026-01-26", appInfo: { name: "slint-inline", version: buildInfo.version }, appCapabilities: {} },
  }, "*");
}
setSource(window.openai?.toolOutput ?? window.openai?.toolInput);
try {
  if (window.parent === window) throw new Error("Open this Slint preview inside the chat.");
  await bridgeReady;
  markLoading("bridge-ready");
  const cache = await openRuntimeCache();
  const cacheKey = runtimeInfo.javascriptUri + ":" + runtimeInfo.wasmHash;
  let cached = await cache?.read(cacheKey);
  if (cached) {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", cached.wasm)), byte => byte.toString(16).padStart(2, "0")).join("");
    if (hash !== runtimeInfo.wasmHash) { cached = undefined; markLoading("runtime-cache-rejected"); }
  }
  markLoading(cached ? "runtime-cache-hit" : "runtime-cache-miss");
  const javascript = cached?.javascript ?? (await readRuntimeResource(runtimeInfo.javascriptUri)).contents[0].text;
  markLoading("javascript-read");
  const initializer = javascript.match(/\b(\w+)\s+as\s+default\b/)?.[1];
  if (!initializer) throw new Error("The built Slint runtime has no initializer.");
  const slint = await new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.type = "module";
    window.addEventListener("slint-runtime-module-ready", event => resolve(event.detail), { once: true });
    script.textContent = javascript + `\nwindow.dispatchEvent(new CustomEvent("slint-runtime-module-ready", { detail: { default: ${initializer}, compile_from_string, run_event_loop, register_font_from_memory } }));`;
    script.addEventListener("error", () => reject(new Error("The host could not initialize the Slint runtime module.")));
    document.head.appendChild(script);
  });
  markLoading("javascript-ready");
  let wasm = cached?.wasm;
  if (!wasm) {
    const compressed = await readBytes(runtimeInfo.wasmChunkUris);
    markLoading("wasm-read");
    wasm = await new Response(compressed.stream().pipeThrough(new DecompressionStream("gzip"))).arrayBuffer();
    markLoading("wasm-decompressed");
  }
  const hash = cached ? runtimeInfo.wasmHash : Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", wasm)), byte => byte.toString(16).padStart(2, "0")).join("");
  if (hash !== runtimeInfo.wasmHash) {
    cache?.close();
    throw new Error("The Slint runtime resources do not match this preview.");
  }
  markLoading("wasm-verified");
  const module = await WebAssembly.compile(wasm);
  markLoading("wasm-compiled");
  await slint.default({ module_or_path: module });
  if (cache) {
    if (cached) cache.close();
    else void cache.write({ key: cacheKey, javascript, wasm }).finally(() => cache.close());
  }
  markLoading("runtime-ready");
  runtime = slint;
  await render();
} catch (error) { showError(error); }
