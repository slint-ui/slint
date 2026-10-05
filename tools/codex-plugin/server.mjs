import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, isAbsolute } from "node:path";
import { createInterface } from "node:readline";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const root = dirname(fileURLToPath(import.meta.url));
const { version } = JSON.parse(await readFile(join(root, "plugin.json"), "utf8"));
const runtimeMetadata = JSON.parse(await readFile(join(root, "runtime/runtime.json"), "utf8"));
const runtimeVersion = runtimeMetadata.version;
const uiUri = `ui://slint/preview/v${version}.html`;
const example = await readFile(join(root, "examples/button.slint"), "utf8");
const component = await readFile(join(root, "components/slint-button.slint"), "utf8");
const runtimeJavascript = await readFile(join(root, "runtime/wasm/slint_wasm_interpreter.js"));
const runtimeWasm = await readFile(join(root, "runtime/wasm/slint_wasm_interpreter_bg.wasm"));
const initializer = runtimeJavascript.toString("utf8").match(/\b(\w+)\s+as\s+default\b/)?.[1];
if (!initializer) throw new Error("The built Wasm module has no default initializer.");
const runtimeScript = runtimeJavascript.toString("utf8") + `\nwindow.slintRuntime = { default: ${initializer}, compile_from_string, run_event_loop };`;
const html = (await readFile(join(root, "runtime/preview.html"), "utf8"))
  .replace("__SLINT_RUNTIME_METADATA__", JSON.stringify(runtimeMetadata))
  .replace("__SLINT_RUNTIME_JAVASCRIPT__", () => runtimeScript.replaceAll("</script", "<\\/script"))
  .replace("__SLINT_RUNTIME_WASM__", runtimeWasm.toString("base64"));
const icon = await readFile(join(root, "assets/slint.svg"));
const icons = [{ src: "data:image/svg+xml;base64," + icon.toString("base64"), mimeType: "image/svg+xml", sizes: ["64x64", "any"] }];
const run = promisify(execFile);
const presentation = { ui: { resourceUri: uiUri }, "openai/outputTemplate": uiUri };
const sourceSchema = {
  type: "object",
  properties: {
    source: { type: "string", minLength: 1, maxLength: 65536 },
    revision: { type: "integer", minimum: 1 },
    width: { type: "integer", minimum: 64, maximum: 2048, default: 320 },
    height: { type: "integer", minimum: 64, maximum: 2048, default: 160 },
  },
  required: ["source", "revision"],
  additionalProperties: false,
};
const renderOutputSchema = {
  type: "object",
  properties: {
    source: { type: "string" }, revision: { type: "integer" },
    width: { type: "integer" }, height: { type: "integer" },
    sourceHash: { type: "string" }, runtimeVersion: { type: "string" }, runtimeRevision: { type: "string" },
  },
  required: ["source", "revision", "width", "height", "sourceHash", "runtimeVersion", "runtimeRevision"],
  additionalProperties: false,
};
const tools = [
  {
    name: "validate_slint", title: "Validate Slint Source", icons,
    description: "Validate a saved Slint file with the language server built from this plugin's monorepo checkout. Return diagnostics, source hash, and revision. The bundled Button import is resolved automatically. Provide an absolute source path. No environment discovery is needed by the agent.",
    inputSchema: { type: "object", properties: { path: { type: "string" }, revision: { type: "integer", minimum: 1 } }, required: ["path", "revision"], additionalProperties: false },
    outputSchema: { type: "object", properties: { status: { type: "string" }, diagnostics: { type: "array" }, sourceHash: { type: "string" }, revision: { type: "integer" }, runtimeVersion: { type: "string" }, message: { type: "string" } }, required: ["status"] },
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
  },
  {
    name: "render_slint", title: "Render Slint Source", icons,
    description: "Preview exact Slint source using the Wasm interpreter built from the same monorepo checkout as the validator. Read or edit source with apply_patch, use validate_slint, then render the same bytes and revision in one execution. Preserve the user's design and match dimensions to the Window. Only slint-button.slint is supplied as a bundled custom import. Source edits requested through the UI arrive in slintEdit model context. The CLI returns source metadata without displaying an inline UI. Starter:\n" + example,
    inputSchema: sourceSchema,
    outputSchema: renderOutputSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    _meta: presentation,
  },
  {
    name: "show_slint_button", title: "Show Slint Button", icons,
    description: "Show the transparent Window and reusable Button starter. Use render_slint for subsequent source changes.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
    outputSchema: renderOutputSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    _meta: presentation,
  },
];

function render(args) {
  const { source, revision, width = 320, height = 160 } = args;
  if (typeof source !== "string" || !source.length || source.length > 65536 ||
      !Number.isSafeInteger(revision) || revision < 1 ||
      !Number.isSafeInteger(width) || width < 64 || width > 2048 ||
      !Number.isSafeInteger(height) || height < 64 || height > 2048) {
    throw new Error("Provide Slint source, a positive revision, and dimensions from 64 to 2048 pixels.");
  }
  const structuredContent = { source, revision, width, height, sourceHash: createHash("sha256").update(source).digest("hex"), runtimeVersion, runtimeRevision: runtimeMetadata.revision };
  return { structuredContent, content: [{ type: "text", text: "Slint preview submitted." }], _meta: presentation };
}

async function handle(message) {
  switch (message.method) {
    case "initialize":
      return { protocolVersion: message.params?.protocolVersion ?? "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "slint", title: "Slint", version, icons } };
    case "ping": return {};
    case "tools/list": return { tools };
    case "resources/list": return { resources: [
      { uri: uiUri, name: "slint-preview", title: "Slint Preview", mimeType: "text/html;profile=mcp-app" },
      { uri: "slint://components/button.slint", name: "slint-button", mimeType: "text/plain" },
    ] };
    case "resources/templates/list": return { resourceTemplates: [] };
    case "resources/read": {
      const uri = message.params?.uri;
      if (uri === "slint://components/button.slint") return { contents: [{ uri, mimeType: "text/plain", text: component }] };
      if (uri !== uiUri) throw new Error("Unknown Slint resource.");
      return { contents: [{ uri, mimeType: "text/html;profile=mcp-app", text: html, _meta: {
        ui: { prefersBorder: true },
        "openai/ui": { availableDisplayModes: ["inline"] },
      } }] };
    }
    case "tools/call": {
      const args = message.params?.arguments ?? {};
      try {
        if (message.params?.name === "show_slint_button") return render({ source: example, revision: 1 });
        if (message.params?.name === "render_slint") return render(args);
        if (message.params?.name !== "validate_slint") throw new Error("Unknown Slint tool.");
        if (typeof args.path !== "string" || !isAbsolute(args.path) || !Number.isSafeInteger(args.revision) || args.revision < 1) throw new Error("Provide an absolute source path and positive revision.");
        const { stdout } = await run(process.env.SLINT_PYTHON_BIN || "python3", [join(root, "scripts/check-source.py"), args.path, "--revision", String(args.revision)], { timeout: 45000, maxBuffer: 1024 * 1024, windowsHide: true });
        const result = JSON.parse(stdout);
        return { structuredContent: result, content: [{ type: "text", text: stdout.trim() }] };
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
