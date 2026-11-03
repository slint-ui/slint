// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore swiftshader

// Runs a browser app in headless Chrome, follows the steps of a JSON file, and prints the page's console.
// Serves the folders itself and drives Chrome over the DevTools protocol, with no npm packages (Node 22+).
//
//   node run_in_chrome.mjs <steps.json> <folder>... [?query]
//
// The first folder that has a file serves it, such as the project's wwwroot, then the build's.
// Each step is one of:
//   { "wait": "<regex>" }                 until a console line matches, at most 60 s
//   { "sleep": <ms> }
//   { "click": [x, y] }                   in CSS pixels
//   { "tap": [x, y] }                     a touch
//   { "type": "<text>" }                  one key press per character
//   { "key": "Enter" }                    a DOM KeyboardEvent.key, pressed and released
//   { "keyDown": "Meta" }, { "keyUp": "Meta" }
//     with "modifiers" (Alt 1, Ctrl 2, Meta 4, Shift 8) and "commands", such as ["copy"], for the browser's editing commands;
//     the key "Mod" and the modifier "Mod" are the shortcut key: Meta on macOS, and Control elsewhere
//   { "insertText": "<text>" }            text without key events, as a phone's keyboard types
//   { "compose": "<text>" }               the composition of an input method; an insertText commits it
//   { "screenshot": "<file.png>" }
// The run fails on an exception, a line with "EXCEPTION", or a wait that times out.
// CHROME may point at a Chrome or Chromium binary; the default is the macOS install location.
// The viewport is VIEWPORT, 800x600 by default, with the device pixel ratio of SCALE, 1 by default; MOBILE=1 emulates a phone with touch.
// LOCALE, such as de-DE, sets the browser's language.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const args = process.argv.slice(2);
const query = args.at(-1)?.startsWith('?') ? args.pop() : '';
const [stepsFile, ...folders] = args;
const steps = JSON.parse(await readFile(stepsFile, 'utf8'));
const roots = folders.map(f => resolve(f));
const chromePath = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json',
  '.wasm': 'application/wasm', '.dat': 'application/octet-stream' };

const server = createServer(async (req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^(\.\.[/\\])+/, '');
  for (const root of roots) {
    const file = join(root, path.endsWith('/') ? path + 'index.html' : path);
    if (!file.startsWith(root)) break;
    try {
      const body = await readFile(file);
      return res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' }).end(body);
    } catch { }
  }
  console.log(`404 ${req.url}`);
  res.writeHead(404).end();
}).listen(0, '127.0.0.1');
await new Promise(r => server.once('listening', r));
const url = `http://127.0.0.1:${server.address().port}/${query}`;

const sleep = ms => new Promise(r => setTimeout(r, ms));
const profile = await mkdtemp(join(tmpdir(), 'slint-browser-'));
const port = 9300 + Math.floor(Math.random() * 500);
// WebGL renders with SwiftShader, as on a machine without a GPU, such as in CI.
// On Linux, CI runners don't allow Chrome's sandbox, and their /dev/shm is small.
const linux = process.platform === 'linux' ? ['--no-sandbox', '--disable-dev-shm-usage'] : [];
const chrome = spawn(chromePath, ['--headless=new', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', ...linux,
  `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
let chromeErrors = '';
chrome.stderr.on('data', data => { chromeErrors += data; });
chrome.on('error', error => { chromeErrors += error.message; });

let status = 1;
try {
  let targets;
  for (let i = 0; i < 300 && !targets; i++) { try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); } catch { await sleep(100); } }
  if (!targets) throw new Error(`Chrome didn't start at ${chromePath}:\n${chromeErrors}`);
  const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
  await new Promise(r => ws.onopen = r);
  let id = 0;
  const pending = new Map();
  const lines = [];
  const waiters = new Set();
  let failed;
  const failure = new Promise(r => failed = r);
  const start = Date.now();
  const print = text => {
    console.log(`${((Date.now() - start) / 1000).toFixed(1).padStart(5)}s ${text}`);
    lines.push(text);
    if (/EXCEPTION/.test(text)) failed(text);
    for (const waiter of waiters) waiter();
  };
  ws.onmessage = m => {
    const d = JSON.parse(m.data);
    if (d.id) { pending.get(d.id)?.(d); pending.delete(d.id); }
    else if (d.method === 'Runtime.consoleAPICalled') print(`[${d.params.type}] ${d.params.args.map(a => a.value ?? a.description).join(' ')}`);
    else if (d.method === 'Runtime.exceptionThrown') print(`EXCEPTION ${d.params.exceptionDetails.exception?.description ?? d.params.exceptionDetails.text}`);
  };
  const send = (method, params = {}) => new Promise(r => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
  // Waits for a line, from the first line not matched yet.
  let seen = 0;
  const waitFor = regex => new Promise((ok, fail) => {
    const check = () => {
      const index = lines.findIndex((line, i) => i >= seen && regex.test(line));
      if (index >= 0) { seen = index + 1; waiters.delete(check); clearTimeout(timer); ok(); }
    };
    const timer = setTimeout(() => { waiters.delete(check); fail(new Error(`no line matched ${regex} in 60 s`)); }, 60000);
    waiters.add(check);
    check();
  });
  const apple = process.platform === 'darwin';
  const shortcut = (key, modifiers) => [key === 'Mod' ? (apple ? 'Meta' : 'Control') : key, modifiers === 'Mod' ? (apple ? 4 : 2) : modifiers];
  const keyDown = (name, { text, modifiers: mods, commands } = {}) => {
    const [key, modifiers] = shortcut(name, mods);
    return send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', key, text, modifiers, commands });
  };
  const keyUp = (name, { modifiers: mods } = {}) => {
    const [key, modifiers] = shortcut(name, mods);
    return send('Input.dispatchKeyEvent', { type: 'keyUp', key, modifiers });
  };
  const key = async (key, text) => {
    await keyDown(key, { text });
    await keyUp(key);
  };
  const run = async () => {
    for (const step of steps) {
      if (step.wait) await waitFor(new RegExp(step.wait));
      else if (step.sleep) await sleep(step.sleep);
      else if (step.click) {
        const [x, y] = step.click;
        for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) {
          await send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mousePressed' ? 1 : 0, clickCount: 1 });
        }
      } else if (step.tap) {
        const [x, y] = step.tap;
        await send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
        await send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      } else if (step.type) { for (const c of step.type) await key(c, c); }
      else if (step.key) await key(step.key, step.key === 'Enter' ? '\r' : undefined);
      else if (step.keyDown) await keyDown(step.keyDown, step);
      else if (step.keyUp) await keyUp(step.keyUp, step);
      else if (step.insertText) await send('Input.insertText', { text: step.insertText });
      else if (step.compose) await send('Input.imeSetComposition', { text: step.compose, selectionStart: step.compose.length, selectionEnd: step.compose.length });
      else if (step.screenshot) {
        const { result } = await send('Page.captureScreenshot', { format: 'png' });
        await writeFile(step.screenshot, Buffer.from(result.data, 'base64'));
        print(`screenshot ${step.screenshot}`);
      }
    }
  };
  await send('Runtime.enable');
  const mobile = process.env.MOBILE === '1';
  const [width, height] = (process.env.VIEWPORT ?? '800x600').split('x').map(Number);
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: Number(process.env.SCALE ?? 1), mobile });
  if (mobile) await send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  if (process.env.LOCALE) {
    await send('Emulation.setLocaleOverride', { locale: process.env.LOCALE });
    // navigator.language, which .NET takes its culture from.
    const { result } = await send('Runtime.evaluate', { expression: 'navigator.userAgent', returnByValue: true });
    await send('Emulation.setUserAgentOverride', { userAgent: result.result.value, acceptLanguage: process.env.LOCALE });
  }
  await send('Page.navigate', { url });
  const error = await Promise.race([run().then(() => null, e => e.message), failure]);
  if (error) console.log(`FAILED: ${error}`);
  status = error ? 1 : 0;
  ws.close();
} finally {
  chrome.kill();
  server.close();
  await rm(profile, { recursive: true, force: true }).catch(() => {});
}
process.exit(status);
