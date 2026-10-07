// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { build } from "esbuild";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const runtime = process.env.SLINT_PLUGIN_RUNTIME_DIR || join(root, "runtime");
const { version } = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
const bundle = await build({
  entryPoints: [join(root, "preview/main.mjs")],
  bundle: true,
  write: false,
  format: "esm",
  platform: "browser",
  target: "es2022",
  minify: true,
  legalComments: "none",
});
let html = await readFile(join(root, "preview/index.html"), "utf8");
const json = value => JSON.stringify(value).replaceAll("<", "\\u003c");
const icon = await readFile(join(root, "assets/slint.svg"));
await writeFile(join(runtime, "slint.svg"), icon);
const assets = {
  __SLINT_PREVIEW_SCRIPT__: bundle.outputFiles[0].text.replace(/<\/script/gi, "<\\/script"),
  __SLINT_ICON__: "data:image/svg+xml;base64," + icon.toString("base64"),
  __SLINT_CODE_FONT__: "data:font/woff2;base64," + (await readFile(join(root, "assets/jetbrains-mono.woff2"))).toString("base64"),
  __SLINT_EXAMPLE_SOURCE__: json(await readFile(join(root, "examples/button.slint"), "utf8")),
  __SLINT_BUTTON_SOURCE__: json(await readFile(join(root, "components/slint-button.slint"), "utf8")),
};
for (const [token, value] of Object.entries(assets)) {
  if (!html.includes(token)) throw new Error(`Preview template is missing ${token}.`);
  html = html.replace(token, () => value);
}
const digest = createHash("sha256").update(html);
for (const file of ["package.json", "server.mjs", "project.mjs", "runtime-assets.mjs", "scripts/check-source.py"]) digest.update(await readFile(join(root, file)));
for (const file of ["runtime.json", "wasm/slint_wasm_interpreter.js", "wasm/slint_wasm_interpreter_bg.wasm"]) digest.update(await readFile(join(runtime, file)));
const metadata = { version, buildId: digest.digest("hex").slice(0, 12) };
await writeFile(join(runtime, "preview.html"), html.replace("__SLINT_BUILD_METADATA__", json(metadata)));
console.log(JSON.stringify(metadata));
