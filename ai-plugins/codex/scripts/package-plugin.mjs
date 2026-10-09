// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore compresslevel rglob writestr
import { execFileSync } from "node:child_process";
import { access, copyFile, cp, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const { version } = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
const runtime = JSON.parse(await readFile(join(root, "runtime/runtime.json"), "utf8"));
if (runtime.platform !== process.platform || runtime.architecture !== process.arch) throw new Error("Package the runtime on the platform recorded by its build.");
const executables = ["slint-lsp", "slint-editor-mcp"].map(name => process.platform === "win32" ? `${name}.exe` : name);
for (const executable of executables) {
  const path = join(root, "runtime", executable);
  const information = await stat(path);
  if (!information.isFile() || !information.size) throw new Error(`Missing runtime executable: ${executable}`);
  await access(path, constants.X_OK);
}
const output = resolve(process.argv[2] || join(root, "runtime", `Slint-v${version}-${runtime.platform}-${runtime.architecture}.zip`));
const staging = await mkdtemp(join(tmpdir(), "slint-package-"));
try {
  const marketplace = join(staging, "slint");
  const plugin = join(marketplace, "plugins/slint");
  const sharedRoot = dirname(root);
  for (const file of ["plugin.json", "mcp_config.json", "icon.svg", ".codex-plugin/plugin.json", ".claude-plugin/plugin.json", ".cursor-plugin/plugin.json"]) {
    await mkdir(dirname(join(plugin, file)), { recursive: true });
    await copyFile(join(sharedRoot, file), join(plugin, file));
  }
  await cp(join(sharedRoot, "skills"), join(plugin, "skills"), { recursive: true });
  const files = ["package.json", "mcp.json", "server.mjs", "editor-mcp.mjs", "views.mjs", "captures.mjs", "project.mjs", "runtime-assets.mjs", "runtime/slint.svg", "components/slint-button.slint", "examples/button.slint", "scripts/check-source.py", "THIRD_PARTY_NOTICES.txt", "runtime/runtime.json", "runtime/preview.html", ...executables.map(executable => `runtime/${executable}`), "runtime/wasm/slint_wasm_interpreter_bg.wasm"];
  for (const file of files) {
    await mkdir(dirname(join(plugin, "codex", file)), { recursive: true });
    await copyFile(join(root, file), join(plugin, "codex", file));
  }
  const catalog = { name: "slint", interface: { displayName: "Slint" }, plugins: [{ name: "slint", source: { source: "local", path: "./plugins/slint" }, policy: { installation: "AVAILABLE", authentication: "ON_INSTALL" }, category: "Developer tools" }] };
  await mkdir(join(marketplace, ".agents/plugins"), { recursive: true });
  await writeFile(join(marketplace, ".agents/plugins/marketplace.json"), JSON.stringify(catalog, null, 2) + "\n");
  await writeFile(join(marketplace, "README.md"), `# Slint Codex plugin v${version}\n\nRuntime: ${runtime.platform}/${runtime.architecture}, Slint revision ${runtime.revision}.\nUse Node.js 20+ and Python 3.\nExtract this folder and keep it in place.\nRun \`codex plugin marketplace add .\` and \`codex plugin add slint@slint\`.\nRestart Codex after installation.\nThe package includes the Slint skills, native LSP, Wasm preview, and Visual Editor bridge.\nInstall a compatible Visual Editor separately and explicitly ask Codex to connect this chat.\nOther platforms build from the monorepo.\n`);
  await mkdir(dirname(output), { recursive: true });
  execFileSync(process.env.SLINT_PYTHON_BIN || "python3", ["-c", "import pathlib,sys,zipfile,stat\nroot=pathlib.Path(sys.argv[1])\nwith zipfile.ZipFile(sys.argv[2],'w',zipfile.ZIP_DEFLATED,compresslevel=9) as archive:\n for path in sorted(root.rglob('*')):\n  if path.is_file():\n   info=zipfile.ZipInfo(path.relative_to(root).as_posix(),(1980,1,1,0,0,0))\n   info.compress_type=zipfile.ZIP_DEFLATED\n   info.external_attr=((stat.S_IFREG | (0o755 if path.name in sys.argv[3:] else 0o644)) << 16)\n   archive.writestr(info,path.read_bytes(),compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)\n", staging, output, ...executables]);
  console.log(output);
} finally {
  await rm(staging, { recursive: true, force: true });
}
