// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import initialize, { compile_from_string, run_event_loop, register_font_from_memory } from "slint-runtime";
import { highlight } from "./syntax.mjs";
import { installPreviewZoom } from "./zoom.mjs";
import { openRuntimeCache } from "./runtime-cache.mjs";

const buildInfo = JSON.parse(document.getElementById("slint-build").textContent);
const runtimeInfo = JSON.parse(document.getElementById("slint-runtime").textContent);
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
let desired;
let generation = 0;
let appliedGeneration = -1;
let rendering = false;
let runtime;
let instance;
let pendingCapture;
let previewDiagnostics = [];
let openedFile;
let fileRevision = 0;
let loadingFile = false;
let fileChanged = false;
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
  const source = desired?.source ?? "";
  const request = ++highlightRequest;
  const pre = document.createElement("pre");
  const code = document.createElement("code");
  code.textContent = source;
  pre.appendChild(code);
  codePanel.replaceChildren(pre);
  if (codePanel.hidden) return;
  void highlight(source, (hostTheme ?? (colorScheme.matches ? "dark" : "light")) === "dark" ? "dark-slint" : "light-slint").then(html => {
    if (request !== highlightRequest) return;
    codePanel.innerHTML = html;
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
zoom = installPreviewZoom({ canvas, initialSize: { width: 320, height: 160 } });
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
    params: { structuredContent: { slintPreview: { previewVersion: buildInfo.version, buildId: buildInfo.buildId, runtimeVersion: runtimeInfo.version, runtimeRevision: runtimeInfo.revision, previewId: desired.previewId, screenshot: desired.screenshot, sourcePath: desired.sourcePath, sourceState: desired.sourceState, projectRoot: desired.projectRoot, revision, sourceHash, state, diagnostics } } },
  }, "*");
}
function showError(error, acknowledge = true) {
  status.hidden = false;
  status.textContent = String(error);
  canvas.dataset.stale = String(Boolean(instance));
  console.error(error);
  if (acknowledge && desired) report("error", desired.revision, [{ level: "error", message: String(error) }], desired.sourceHash);
}
async function openFile(file) {
  if (!file?.resourceUri || openedFile?.resourceUri === file.resourceUri) return;
  if (openedFile) await hostRequest("resources/unsubscribe", { uri: openedFile.resourceUri }).catch(() => {});
  openedFile = file;
  await bridgeReady;
  await hostRequest("resources/subscribe", { uri: file.resourceUri }).catch(error => console.error("File updates unavailable", error));
  await reloadFile();
}
async function reloadFile() {
  fileChanged = true;
  if (loadingFile) return;
  loadingFile = true;
  try {
    while (fileChanged) {
      fileChanged = false;
      const file = openedFile;
      const resource = (await readRuntimeResource(file.resourceUri)).contents[0];
      const source = resource.text ?? new TextDecoder().decode(Uint8Array.from(atob(resource.blob), char => char.charCodeAt(0)));
      const result = await hostRequest("tools/call", { name: "load_slint_file_preview", arguments: { source, revision: ++fileRevision } });
      if (file !== openedFile) continue;
      if (result.isError) showError(result.structuredContent.message || result.structuredContent.diagnostics.map(item => item.message).join("\n"), false);
      else receiveResult(result);
    }
  } finally { loadingFile = false; }
}
function receiveResult(result) {
  if (result?._meta?.file) void openFile(result._meta.file).catch(showError);
  setSource({ ...result?.structuredContent, ...result?._meta?.preview, captureToken: result?._meta?.captureToken, captureId: result?._meta?.captureId, automaticSize: result?._meta?.automaticSize });
}
function setSource(input) {
  if (!input?.project?.id || typeof input.source !== "string" || !input.source.length || input.source.length > 65536) return;
  if (!Number.isSafeInteger(input.revision) || input.revision < (desired?.revision ?? 0)) return;
  const next = { width: 320, height: 160, ...input };
  if (!Number.isSafeInteger(next.width) || next.width < 64 || next.width > 2048 ||
      !Number.isSafeInteger(next.height) || next.height < 64 || next.height > 2048) return;
  if (next.revision === desired?.revision) {
    if (next.source !== desired.source) return;
    if (next.width === desired.width && next.height === desired.height) {
      desired = { ...desired, ...next };
      return;
    }
  }
  desired = next;
  updateCode();
  generation += 1;
  void render().catch(showError);
}
function captureFrame() {
  const current = pendingCapture;
  if (!current || current.token !== generation) return;
  pendingCapture = undefined;
  const capturedAt = Date.now();
  try {
    canvas.toBlob(blob => {
      void publishCapture(current, blob, capturedAt).catch(error => console.error("Slint capture unavailable", error));
    }, "image/png");
  } catch (error) {
    void publishCapture(current, undefined, capturedAt, String(error)).catch(error => console.error("Slint capture unavailable", error));
  }
}
async function publishCapture(current, blob, capturedAt, error) {
  if (current.token !== generation) return;
  const args = { previewId: current.previewId, captureToken: current.captureToken, captureId: current.captureId, revision: current.revision, sourceHash: current.sourceHash };
  if (error || !blob) args.error = error || "The canvas did not return an image.";
  else if (blob.size > 4 * 1024 * 1024) args.error = "The canvas capture exceeds 4 MiB.";
  else {
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let binary = "";
    for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
    Object.assign(args, { data: btoa(binary), capturedAt });
  }
  if (current.token !== generation) return;
  let result = await hostRequest("tools/call", { name: "publish_preview_capture", arguments: args });
  if (result.isError && !args.error) {
    const { data, capturedAt, ...identity } = args;
    result = await hostRequest("tools/call", { name: "publish_preview_capture", arguments: { ...identity, error: (result.structuredContent?.message || "The host rejected the capture.").slice(0, 1024) } });
  }
  if (result.isError) throw new Error(result.structuredContent?.message || "The host rejected the capture.");
  if (current.token !== generation) return;
  desired.screenshot = result.structuredContent;
  report("ready", current.revision, current.diagnostics, current.sourceHash);
}
async function render() {
  if (!runtime || !desired || rendering) return;
  rendering = true;
  try {
    while (appliedGeneration !== generation) {
      const current = { ...desired };
      const token = generation;
      const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(current.source));
      current.sourceHash = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
      if (current.source === desired.source) desired.sourceHash = current.sourceHash;
      const project = await prepareProject(current.project, current.source, current.sourceHash);
      const result = await runtime.compile_from_string(current.source, project.baseUrl, async path => {
        const key = decodeURIComponent(new URL(path, project.baseUrl).pathname).replace(project.prefix, "");
        if (project.sources.has(key)) return project.sources.get(key);
        throw new Error("Unknown bundled import: " + path);
      }, project.images);
      const component = result.component;
      const projectPrefix = "file://" + project.prefix;
      const diagnostics = result.diagnostics.map(diagnostic => ({ ...diagnostic,
        fileName: diagnostic.fileName.startsWith(projectPrefix)
          ? current.projectRoot + "/" + decodeURIComponent(diagnostic.fileName.slice(projectPrefix.length)) : diagnostic.fileName,
      }));
      if (token !== generation) { component?.free(); result.free(); continue; }
      if (!component) {
        const errorText = diagnostics.map(({ fileName, lineNumber, message }) => `${fileName}:${lineNumber}: ${message}`).join("\n");
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
        await instance.on_after_rendering(captureFrame);
      }
      await instance.show();
      component.free();
      result.free();
      appliedGeneration = token;
      if (token !== generation) continue;
      canvas.dataset.stale = "false";
      status.hidden = true;
      zoom.setSize(current.width, current.height);
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      if (token !== generation) continue;
      if (current.automaticSize) {
        current.width = Math.round(canvas.width / devicePixelRatio);
        current.height = Math.round(canvas.height / devicePixelRatio);
        canvas.style.width = current.width + "px";
        canvas.style.height = current.height + "px";
        zoom.setSize(current.width, current.height);
      }
      if (current.captureToken) {
        previewDiagnostics = diagnostics;
        pendingCapture = { ...current, token, diagnostics };
        desired.screenshot = { status: "pending" };
        await instance.request_redraw();
      }
      report("ready", current.revision, diagnostics, current.sourceHash);
      if (current.captureId) void watchCaptureRequests(current, token).catch(error => console.error("Live capture unavailable", error));
    }
  } finally { rendering = false; }
}
async function watchCaptureRequests(current, token) {
  let after = current.captureId;
  while (token === generation) {
    const uri = `slint://capture/${current.previewId}/${after}/next`;
    const result = await readRuntimeResource(uri);
    if (token !== generation) return;
    const request = JSON.parse(result.contents[0].text);
    if (request.status === "unavailable") return;
    if (request.status === "idle") continue;
    after = request.captureId;
    if (request.revision !== desired.revision || request.sourceHash !== desired.sourceHash) throw new Error("The live capture request does not match the displayed source.");
    pendingCapture = { ...desired, captureId: after, token, diagnostics: previewDiagnostics };
    desired.screenshot = { status: "pending", captureId: after };
    await instance.request_redraw();
  }
}
let resolveBridge;
let rejectBridge;
const bridgeReady = new Promise((resolve, reject) => { resolveBridge = resolve; rejectBridge = reject; });
const resourceRequests = new Map();
let resourceRequestId = 0;
function hostRequest(method, params) {
  return new Promise((resolve, reject) => {
    const id = `slint-runtime-${++resourceRequestId}`;
    const timeout = setTimeout(() => { resourceRequests.delete(id); reject(new Error("The host did not respond to the Slint preview request.")); }, 30000);
    resourceRequests.set(id, { resolve, reject, timeout });
    window.parent.postMessage({ jsonrpc: "2.0", id, method, params }, "*");
  });
}
const readRuntimeResource = uri => hostRequest("resources/read", { uri });
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
async function prepareProject(project, source, sourceHash) {
  const prefix = `/__slint_preview/${project.id}/`;
  const baseUrl = "file://" + prefix + project.entry.split("/").map(encodeURIComponent).join("/");
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
  if (message.method === "ui/notifications/tool-input") {
    if (message.params?.arguments?.file) void openFile(message.params.arguments.file).catch(showError);
    else setSource(message.params?.arguments);
  }
  if (message.method === "notifications/resources/updated" && message.params?.uri === openedFile?.resourceUri) void reloadFile().catch(showError);
  if (message.method === "ui/notifications/tool-result") receiveResult(message.params);
});
if (window.parent !== window) {
  window.parent.postMessage({
    jsonrpc: "2.0", id: "slint-init", method: "ui/initialize",
    params: { protocolVersion: "2026-01-26", appInfo: { name: "slint-inline", version: buildInfo.version }, appCapabilities: {} },
  }, "*");
}
const legacyMetadata = window.openai?.toolResponseMetadata;
const legacyResult = legacyMetadata?.mcp_tool_result ?? legacyMetadata?.call_tool_result;
const legacyUi = legacyResult?._meta ?? legacyMetadata;
receiveResult({ structuredContent: window.openai?.toolOutput, _meta: legacyUi });
try {
  if (window.parent === window) throw new Error("Open this Slint preview inside the chat.");
  await bridgeReady;
  const cache = await openRuntimeCache();
  const cacheKey = runtimeInfo.wasmHash;
  let cached = await cache?.read(cacheKey);
  if (cached) {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", cached.wasm)), byte => byte.toString(16).padStart(2, "0")).join("");
    if (hash !== runtimeInfo.wasmHash) cached = undefined;
  }
  let wasm = cached?.wasm;
  if (!wasm) {
    const compressed = await readBytes(runtimeInfo.wasmChunkUris);
    wasm = await new Response(compressed.stream().pipeThrough(new DecompressionStream("gzip"))).arrayBuffer();
  }
  const hash = cached ? runtimeInfo.wasmHash : Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", wasm)), byte => byte.toString(16).padStart(2, "0")).join("");
  if (hash !== runtimeInfo.wasmHash) {
    cache?.close();
    throw new Error("The Slint runtime resources do not match this preview.");
  }
  const module = await WebAssembly.compile(wasm);
  await initialize({ module_or_path: module });
  if (cache) {
    if (cached) cache.close();
    else void cache.write({ key: cacheKey, wasm }).finally(() => cache.close());
  }
  runtime = { compile_from_string, run_event_loop, register_font_from_memory };
  await render();
} catch (error) { showError(error); }
