import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, readdir, realpath, rename, stat, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";

const cache = join(tmpdir(), `slint-codex-previews-${process.getuid?.() ?? process.env.USERNAME ?? "user"}`);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const types = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".svg": "image/svg+xml", ".webp": "image/webp", ".ttf": "font/ttf", ".otf": "font/otf" };

function references(source) {
  const tokens = Array.from(source.matchAll(/\/\*[\s\S]*?\*\/|\/\/[^\n]*|"(?:\\.|[^"\\])*"|[A-Za-z_][\w-]*|[^\s]/g), match => match[0]).filter(token => !token.startsWith("//") && !token.startsWith("/*"));
  const paths = [];
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i] === "import") {
      let end = i + 1;
      while (end < tokens.length && tokens[end] !== ";") end++;
      if (tokens[end - 1]?.startsWith('"')) paths.push(JSON.parse(tokens[end - 1]));
      i = end;
    } else if (tokens[i] === "@" && tokens[i + 1] === "image-url" && tokens[i + 2] === "(" && tokens[i + 3]?.startsWith('"')) {
      paths.push(JSON.parse(tokens[i + 3]));
    }
  }
  return paths;
}

export async function snapshotProject(path, projectRoot, buttonSource, validatedSourceHash) {
  if (!isAbsolute(path) || (projectRoot && !isAbsolute(projectRoot))) throw new Error("Source path and project root must be absolute.");
  const sourcePath = await realpath(path);
  const root = await realpath(projectRoot || dirname(sourcePath));
  const files = {};
  let total = 0;
  const keyFor = absolute => {
    const key = relative(root, absolute);
    if (key === ".." || key.startsWith(".." + sep) || isAbsolute(key)) throw new Error("A preview dependency is outside the declared project root.");
    return key.split(sep).join("/");
  };
  async function visit(absolute, bundled = false) {
    const actual = bundled ? absolute : await realpath(absolute);
    keyFor(actual);
    const key = keyFor(absolute);
    if (files[key]) return;
    if (Object.keys(files).length >= 128) throw new Error("A preview supports at most 128 dependency files.");
    const extension = extname(actual).toLowerCase();
    if (extension !== ".slint" && !types[extension]) throw new Error(`Unsupported preview dependency: ${key}`);
    const bytes = bundled ? Buffer.from(buttonSource) : await readFile(actual);
    total += bytes.length;
    if (bytes.length > 8 * 1024 * 1024 || total > 16 * 1024 * 1024) throw new Error("Preview dependencies exceed the 16 MiB total or 8 MiB per-file limit.");
    files[key] = { mimeType: extension === ".slint" ? "text/plain" : types[extension], data: bytes.toString("base64"), hash: hash(bytes) };
    if (extension !== ".slint") return;
    if (bytes.length > 65536) throw new Error("Each Slint source file must be at most 64 KiB.");
    for (const dependency of references(bytes.toString("utf8"))) {
      if (dependency === "std-widgets.slint" || dependency.startsWith("data:")) continue;
      if (isAbsolute(dependency)) throw new Error("Preview dependencies must use relative project paths.");
      if (/^[a-z][a-z\d+.-]*:/i.test(dependency)) throw new Error("Preview dependencies must be local project files.");
      const candidate = resolve(dirname(absolute), dependency);
      keyFor(candidate);
      let missing = false;
      try { await stat(candidate); } catch (error) { if (error.code !== "ENOENT") throw error; missing = true; }
      await visit(candidate, missing && dependency === "slint-button.slint");
    }
  }
  await visit(sourcePath);
  const entry = keyFor(sourcePath);
  if (validatedSourceHash !== undefined && files[entry].hash !== validatedSourceHash) throw new Error("The saved source changed after validation. Validate it again before rendering.");
  const snapshot = { sourcePath, projectRoot: root, entry, files };
  const id = hash(JSON.stringify(snapshot));
  await mkdir(cache, { recursive: true, mode: 0o700 });
  const cacheInfo = await lstat(cache);
  if (!cacheInfo.isDirectory() || cacheInfo.isSymbolicLink()) throw new Error("Invalid project snapshot cache directory.");
  const destination = join(cache, id + ".json");
  const temporary = destination + "." + process.pid;
  await writeFile(temporary, JSON.stringify(snapshot), { mode: 0o600 });
  await rename(temporary, destination);
  const existing = await Promise.all((await readdir(cache)).filter(name => /^[a-f\d]{64}\.json$/.test(name)).map(async name => {
    try { return { name, modified: (await stat(join(cache, name))).mtimeMs }; }
    catch (error) { if (error.code !== "ENOENT") throw error; return undefined; }
  }));
  for (const old of existing.filter(Boolean).sort((a, b) => b.modified - a.modified).slice(32)) await unlink(join(cache, old.name)).catch(error => { if (error.code !== "ENOENT") throw error; });
  return describeSnapshot(id, snapshot);
}

function describeSnapshot(id, snapshot) {
  const files = Object.fromEntries(Object.entries(snapshot.files).map(([name, file]) => {
    const bytes = Buffer.from(file.data, "base64");
    return [name, { mimeType: file.mimeType, hash: file.hash, uris: Array.from({ length: Math.ceil(bytes.length / (256 * 1024)) || 1 }, (_, index) => `slint://project/${id}/${encodeURIComponent(name)}/${index}`) }];
  }));
  return { id, sourcePath: snapshot.sourcePath, projectRoot: snapshot.projectRoot, entry: snapshot.entry, files, source: Buffer.from(snapshot.files[snapshot.entry].data, "base64").toString("utf8") };
}

export async function readProjectResource(uri) {
  const match = /^slint:\/\/project\/([a-f\d]{64})\/([^/]+)\/(\d+)$/.exec(uri);
  if (!match) throw new Error("Unknown project resource.");
  let snapshot;
  try { snapshot = JSON.parse(await readFile(join(cache, match[1] + ".json"), "utf8")); }
  catch (error) { if (error.code === "ENOENT") throw new Error("This project preview snapshot has expired. Render the saved file again."); throw error; }
  const name = decodeURIComponent(match[2]);
  const file = Object.hasOwn(snapshot.files, name) ? snapshot.files[name] : undefined;
  if (!file) throw new Error("Unknown project dependency.");
  const bytes = Buffer.from(file.data, "base64");
  const index = Number(match[3]);
  if (!Number.isSafeInteger(index) || index >= Math.max(1, Math.ceil(bytes.length / (256 * 1024)))) throw new Error("Unknown project resource chunk.");
  return { uri, mimeType: file.mimeType, blob: bytes.subarray(index * 256 * 1024, (index + 1) * 256 * 1024).toString("base64") };
}
