// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * Runs inside the preview iframe. Receives a "run" postMessage from the parent
 * with the user's JS and the in-memory .slint files, then executes the JS
 * with `slint` and `playground` exposed as globals.
 *
 * The user's JS is loaded via a Blob URL + dynamic `import()` so it can use
 * top-level `await`. Bare imports (`import * as slint from ...`) would not
 * resolve inside a Blob module, which is why we expose `slint` as a global
 * instead.
 */

import * as slint from "slint-ui-browser";

interface RunMessage {
    type: "run";
    userJs: string;
    slintFiles: Record<string, string>;
    mainSlint: string;
    mainUrl: string;
    canvasWidth: number;
    canvasHeight: number;
}

const errorEl = document.getElementById("error-overlay") as HTMLPreElement;
function showError(message: string) {
    errorEl.textContent = message;
    errorEl.classList.add("visible");
    parent.postMessage({ type: "error", message }, "*");
    forwardLog("error", message);
}

function clearError() {
    errorEl.textContent = "";
    errorEl.classList.remove("visible");
}

function formatLogArg(v: unknown): string {
    if (typeof v === "string") return v;
    if (v instanceof Error) return v.stack ?? v.message;
    if (typeof v === "function") return "[Function]";
    try {
        return JSON.stringify(v);
    } catch {
        return String(v);
    }
}

function forwardLog(level: "log" | "warn" | "error", text: string) {
    parent.postMessage({ type: "log", level, text }, "*");
}

// Forward console.* into the parent's Logs tab. Keep the originals so we
// still see them in DevTools.
for (const level of ["log", "warn", "error"] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
        original(...args);
        forwardLog(level, args.map(formatLogArg).join(" "));
    };
}

console.log("[playground] preview iframe loaded");

// Pre-warm the wasm module. First load is ~46 MB (dev build), so it can be
// slow on mobile — heartbeat below makes the wait visible.
const wasmStart = performance.now();
let initDone = false;
console.log("[playground] starting wasm init…");
slint.initWasm().then(
    () => {
        initDone = true;
        const ms = (performance.now() - wasmStart) | 0;
        console.log(`[playground] wasm initialised (${ms} ms)`);
    },
    (err) => showError(`Failed to load wasm: ${err}`),
);

const heartbeat = setInterval(() => {
    if (initDone) {
        clearInterval(heartbeat);
        return;
    }
    const s = ((performance.now() - wasmStart) / 1000) | 0;
    console.log(`[playground] still waiting for wasm… (${s}s)`);
}, 2000);

window.addEventListener("error", (e) => showError(`${e.message}\n${e.error?.stack ?? ""}`));
window.addEventListener("unhandledrejection", (e) => {
    const r = e.reason as { message?: string; stack?: string } | undefined;
    showError(`Unhandled: ${r?.message ?? r}\n${r?.stack ?? ""}`);
});

let canvasResizeObserver: ResizeObserver | null = null;

/**
 * Set up the canvas at its preferred size and scale the wrapping div to fit
 * the iframe. We use HTML width/height attributes (not CSS) on the canvas
 * because slint's winit/web backend only honours the canvas's existing CSS
 * size when computed width/height are "auto" — see canvas_has_explicit_size_set
 * in internal/backends/winit/winitwindowadapter.rs. Any inline or rule-based
 * CSS sizing on the canvas makes slint resize down to its own preferred size.
 */
function setupCanvasFit(width: number, height: number): void {
    const root = document.getElementById("preview-root");
    const fit = document.getElementById("canvas-fit") as HTMLElement | null;
    const canvas = document.getElementById("canvas") as HTMLCanvasElement | null;
    if (!root || !canvas || !fit) return;

    canvas.width = width;
    canvas.height = height;

    const apply = () => {
        const W = root.clientWidth;
        const H = root.clientHeight;
        if (W === 0 || H === 0) return;
        const scale = Math.min(W / width, H / height);
        fit.style.transform = `scale(${scale})`;
    };

    apply();
    canvasResizeObserver?.disconnect();
    canvasResizeObserver = new ResizeObserver(apply);
    canvasResizeObserver.observe(root);
}

let currentBlobUrl: string | null = null;

window.addEventListener("message", async (event) => {
    const data = event.data as RunMessage | { type: string };
    if (!data || data.type !== "run") return;
    const msg = data as RunMessage;

    clearError();

    // Render the slint window at canvasWidth × canvasHeight (HTML attrs),
    // then visually scale the wrapper to fit the iframe.
    setupCanvasFit(msg.canvasWidth, msg.canvasHeight);

    const playground = {
        mainSlint: msg.mainSlint,
        mainUrl: msg.mainUrl,
        async readFile(relativePath: string): Promise<string> {
            // Walk the slintFiles map looking for a URL that ends with this
            // relative path — saves the user from constructing /@fs/ URLs.
            for (const [url, content] of Object.entries(msg.slintFiles)) {
                if (url.endsWith("/" + relativePath) || url.endsWith(relativePath)) {
                    return content;
                }
            }
            const r = await fetch(relativePath);
            if (!r.ok) throw new Error(`readFile(${relativePath}): ${r.status}`);
            return await r.text();
        },
        async fileLoader(url: string): Promise<string> {
            if (msg.slintFiles[url] !== undefined) {
                return msg.slintFiles[url];
            }
            const r = await fetch(url);
            if (!r.ok) throw new Error(`fileLoader(${url}): ${r.status}`);
            return await r.text();
        },
    };

    (window as unknown as { slint: typeof slint }).slint = slint;
    (window as unknown as { playground: typeof playground }).playground = playground;

    if (currentBlobUrl) {
        URL.revokeObjectURL(currentBlobUrl);
        currentBlobUrl = null;
    }

    const blob = new Blob([msg.userJs], { type: "application/javascript" });
    currentBlobUrl = URL.createObjectURL(blob);

    parent.postMessage({ type: "running" }, "*");
    console.log("[playground] running main.js");

    try {
        // Hide the dynamic import from vite's static analyser — otherwise it
        // appends `?import` to the Blob URL via __vite__injectQuery, which
        // breaks it. /* @vite-ignore */ alone is not enough in vite 8.
        await dynamicImport(currentBlobUrl);
    } catch (err) {
        const e = err as Error;
        showError(`${e.message}\n${e.stack ?? ""}`);
    }
});

const dynamicImport: (url: string) => Promise<unknown> = new Function(
    "url",
    "return import(url)",
) as (url: string) => Promise<unknown>;

parent.postMessage({ type: "ready" }, "*");
