import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const { version } = JSON.parse(await readFile(join(root, "plugin.json"), "utf8"));
const path = join(root, "preview/index.html");
const pattern = /(<script id="slint-build" type="application\/json">)[\s\S]*?(<\/script>)/;
let original = await readFile(path, "utf8");
const zoom = (await readFile(join(root, "preview/zoom.mjs"), "utf8")).replace(/^export /gm, "");
original = original.replace("__SLINT_ZOOM_SOURCE__", () => zoom);
if (!pattern.test(original)) throw new Error("Preview build metadata is missing.");
const digest = createHash("sha256").update(original.replace(pattern, "$1__BUILD__$2"));
for (const file of ["plugin.json", "server.mjs", "project.mjs", "components/slint-button.slint", "scripts/check-source.py", "runtime/runtime.json", "runtime/wasm/slint_wasm_interpreter.js", "runtime/wasm/slint_wasm_interpreter_bg.wasm"]) digest.update(await readFile(join(root, file)));
const metadata = { version, buildId: digest.digest("hex").slice(0, 12) };
await writeFile(join(root, "runtime/preview.html"), original.replace(pattern, (_, before, after) => before + JSON.stringify(metadata) + after));
console.log(JSON.stringify(metadata));
