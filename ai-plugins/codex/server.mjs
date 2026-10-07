// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { createHash } from "node:crypto";
import { gzipSync } from "node:zlib";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, isAbsolute } from "node:path";
import { createInterface } from "node:readline";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { loadRuntime } from "./runtime-assets.mjs";
import { captureProject, snapshotProject, readProjectResource } from "./project.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const { version } = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
const builtRuntime = await loadRuntime(join(root, "runtime"));
const runtimeMetadata = builtRuntime?.metadata;
const runtimeVersion = runtimeMetadata?.version;
const uiUri = `ui://slint/preview/v${version}.html`;
const example = await readFile(join(root, "examples/button.slint"), "utf8");
const component = await readFile(join(root, "components/slint-button.slint"), "utf8");
const runtimeWasm = builtRuntime?.wasm ?? Buffer.alloc(0);
const wasmHash = createHash("sha256").update(runtimeWasm).digest("hex");
const runtimeResources = new Map();
const compressedWasm = gzipSync(runtimeWasm);
const wasmChunkUris = [];
for (let offset = 0; offset < compressedWasm.length; offset += 256 * 1024) {
  const uri = `slint://runtime/${wasmHash}/wasm/${wasmChunkUris.length}`;
  wasmChunkUris.push(uri);
  runtimeResources.set(uri, { uri, mimeType: "application/octet-stream", blob: compressedWasm.subarray(offset, offset + 256 * 1024).toString("base64") });
}
const html = builtRuntime ? builtRuntime.html
  .replace("__SLINT_RUNTIME_METADATA__", JSON.stringify({ ...runtimeMetadata, wasmChunkUris, wasmHash })) : "";
if (Buffer.byteLength(html) >= 1024 * 1024) throw new Error("The inline Slint HTML must remain smaller than 1 MiB.");
const icon = builtRuntime?.icon ?? await readFile(join(root, "../icon.svg"));
const icons = [{ src: "data:image/svg+xml;base64," + icon.toString("base64"), mimeType: "image/svg+xml", sizes: ["64x64", "any"] }];
const run = promisify(execFile);
const presentation = { ui: { resourceUri: uiUri }, "openai/outputTemplate": uiUri };
const sourceSchema = {
  type: "object",
  properties: {
    path: { type: "string", description: "Absolute saved .slint source path." },
    projectRoot: { type: "string", description: "Absolute root containing relative imports and assets. Defaults to the source directory." },
    validatedProjectHash: { type: "string", pattern: "^[a-f0-9]{64}$", description: "The projectHash returned by validate_slint, covering the entry, dependencies, assets, and runtime revision." },
    revision: { type: "integer", minimum: 1 },
    width: { type: "integer", minimum: 64, maximum: 2048, default: 320 },
    height: { type: "integer", minimum: 64, maximum: 2048, default: 160 },
  },
  required: ["path", "revision", "validatedProjectHash"],
  additionalProperties: false,
};
const renderOutputSchema = {
  type: "object",
  properties: {
    source: { type: "string" }, revision: { type: "integer" },
    width: { type: "integer" }, height: { type: "integer" },
    sourceHash: { type: "string" }, runtimeVersion: { type: "string" }, runtimeRevision: { type: "string" },
    sourcePath: { type: "string" }, projectRoot: { type: "string" }, project: { type: "object" },
  },
  required: ["source", "revision", "width", "height", "sourceHash", "runtimeVersion", "runtimeRevision", "sourcePath", "projectRoot", "project"],
  additionalProperties: false,
};
const tools = [
  {
    name: "validate_slint", title: "Validate Slint Source", icons,
    description: "Validate a saved Slint file with the bundled language server. Success is structuredContent.status === 'valid'. Status is 'error' for Slint errors or 'failure' for validator/setup errors; never check for 'ok'. Return diagnostics, source hash, and revision. The bundled Button import is resolved automatically. Provide an absolute source path and projectRoot for dependencies outside its directory. No environment discovery is needed. Save, validate, and render in one code-mode execution, rendering only after status 'valid'.",
    inputSchema: { type: "object", properties: { path: { type: "string" }, projectRoot: { type: "string" }, revision: { type: "integer", minimum: 1 } }, required: ["path", "revision"], additionalProperties: false },
    outputSchema: { type: "object", properties: { status: { type: "string", enum: ["valid", "error", "failure"], description: "valid means no error diagnostics; error means Slint errors; failure means a validator or setup error." }, diagnostics: { type: "array" }, sourceHash: { type: "string" }, projectHash: { type: "string" }, projectRoot: { type: "string" }, revision: { type: "integer" }, runtimeVersion: { type: "string" }, message: { type: "string" } }, required: ["status"] },
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
  },
  {
    name: "render_slint", title: "Render Slint Source", icons,
    description: "Preview saved Slint source using the matching Wasm interpreter. Provide path plus validatedProjectHash from validate_slint; the server checks the saved project. Use projectRoot for relative component imports, images, and fonts. For simple Buttons, reuse the starter and change only requested properties; preserve centering and state defaults. Save, validate, check status 'valid', and render in one execution. For follow-up edits, use sourcePath, revision, and sourceHash in the preview model context, preserving the same file. A response means submitted; model-context state 'ready' acknowledges display, while 'error' contains frontend diagnostics. Starter:\n" + example,
    inputSchema: sourceSchema,
    outputSchema: renderOutputSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    _meta: presentation,
  },
];

async function render(args) {
  const { revision, width = 320, height = 160 } = args;
  if (!Number.isSafeInteger(revision) || revision < 1 || !Number.isSafeInteger(width) || width < 64 || width > 2048 || !Number.isSafeInteger(height) || height < 64 || height > 2048) throw new Error("Provide a positive revision and dimensions from 64 to 2048 pixels.");
  if (typeof args.path !== "string" || !isAbsolute(args.path) || !args.validatedProjectHash || args.source !== undefined) throw new Error("Provide an absolute saved path and validatedProjectHash before rendering.");
  const { source, ...project } = await snapshotProject(args.path, args.projectRoot, component, args.validatedProjectHash, runtimeMetadata.revision);
  if (typeof source !== "string" || !source.length || Buffer.byteLength(source) > 65536) throw new Error("Provide non-empty Slint source of at most 64 KiB.");
  const structuredContent = { source, revision, width, height, sourceHash: createHash("sha256").update(source).digest("hex"), runtimeVersion, runtimeRevision: runtimeMetadata.revision };
  Object.assign(structuredContent, { sourcePath: project.sourcePath, projectRoot: project.projectRoot, project });
  return { structuredContent, content: [{ type: "text", text: "Slint preview submitted." }], _meta: presentation };
}

async function handle(message) {
  switch (message.method) {
    case "initialize":
      return { protocolVersion: message.params?.protocolVersion ?? "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "slint", title: "Slint", version, icons } };
    case "ping": return {};
    case "tools/list": return { tools: runtimeMetadata ? tools : [] };
    case "resources/list": return { resources: runtimeMetadata ? [
      { uri: uiUri, name: "slint-preview", title: "Slint Preview", mimeType: "text/html;profile=mcp-app" },
      ...Array.from(runtimeResources.values(), ({ uri, mimeType }) => ({ uri, mimeType, name: uri.split("/").slice(-2).join("-") })),
    ] : [] };
    case "resources/templates/list": return { resourceTemplates: [{ uriTemplate: "slint://project/{snapshot}/{file}/{chunk}", name: "project-dependency", description: "Bounded dependency chunks from a submitted project snapshot." }] };
    case "resources/read": {
      const uri = message.params?.uri;
      if (typeof uri === "string" && uri.startsWith("slint://project/")) return { contents: [await readProjectResource(uri)] };
      if (runtimeResources.has(uri)) return { contents: [runtimeResources.get(uri)] };
      if (uri !== uiUri) throw new Error("Unknown Slint resource.");
      return { contents: [{ uri, mimeType: "text/html;profile=mcp-app", text: html, _meta: {
        ui: { prefersBorder: true, csp: { resourceDomains: ["blob:", "data:"] } },
        "openai/ui": { availableDisplayModes: ["inline"] },
      } }] };
    }
    case "tools/call": {
      const args = message.params?.arguments ?? {};
      try {
        if (!runtimeMetadata) throw new Error("Build the Codex runtime in the Slint monorepo or install a platform package to enable preview tools.");
        if (message.params?.name === "render_slint") return await render(args);
        if (message.params?.name !== "validate_slint") throw new Error("Unknown Slint tool.");
        if (typeof args.path !== "string" || !isAbsolute(args.path) || !Number.isSafeInteger(args.revision) || args.revision < 1) throw new Error("Provide an absolute source path and positive revision.");
        let captured;
        let captureError;
        try { captured = await captureProject(args.path, args.projectRoot, component, runtimeMetadata.revision); }
        catch (error) { captureError = error; }
        const { stdout } = await run(process.env.SLINT_PYTHON_BIN || "python3", [join(root, "scripts/check-source.py"), args.path, "--revision", String(args.revision)], { timeout: 45000, maxBuffer: 1024 * 1024, windowsHide: true });
        const result = JSON.parse(stdout);
        if (result.status === "valid") {
          if (captureError) throw captureError;
          const after = await captureProject(args.path, args.projectRoot, component, runtimeMetadata.revision);
          if (captured.id !== after.id || result.sourceHash !== captured.snapshot.files[captured.snapshot.entry].hash) throw new Error("The project changed during validation. Validate it again.");
          if (result.runtimeRevision !== runtimeMetadata.revision) throw new Error("The validator runtime changed. Refresh the Slint plugin.");
          result.projectHash = captured.id;
          result.projectRoot = captured.snapshot.projectRoot;
        }
        return { structuredContent: result, content: [{ type: "text", text: result.status === 'valid' ? 'Slint source validated.' : 'Slint validation returned diagnostics.' }] };
      } catch (error) {
        const output = error.stdout?.trim() || error.stderr?.trim() || error.message;
        let result;
        try { result = JSON.parse(output); } catch { result = { status: "failure", message: output }; }
        return { isError: true, structuredContent: result, content: [{ type: "text", text: JSON.stringify(result) }] };
      }
    }
    default: throw new Error("Unknown MCP method.");
  }
}
for await (const line of createInterface({ input: process.stdin })) {
  let message;
  try {
    message = JSON.parse(line);
    if (message.id === undefined) continue;
    const result = await handle(message);
    process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }) + "\n");
  } catch (error) {
    process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: message?.id ?? null, error: { code: -32602, message: error.message } }) + "\n");
  }
}
