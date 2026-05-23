// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Browser port of demos/printerdemo/node/main.js, using slint-wasm-interpreter.

import * as slint from "slint-wasm-interpreter";

const statusEl = document.getElementById("status");
function setStatus(text, isError = false) {
    statusEl.textContent = text;
    statusEl.classList.toggle("error", isError);
}

// Vite serves files under the project root by default; for workspace-relative
// paths like `../ui/...` we use vite's `/@fs/` escape hatch.
const UI_BASE = new URL("/@fs/workspace/demos/printerdemo/ui/", window.location.origin);

async function fetchText(url) {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`fetch ${url}: ${r.status}`);
    return await r.text();
}

async function main() {
    try {
        setStatus("Compiling .slint sources…");

        const entryUrl = new URL("printerdemo.slint", UI_BASE).href;
        const entrySource = await fetchText(entryUrl);

        const ui = await slint.loadSource(entrySource, entryUrl, {
            fileLoader: async (url) => fetchText(url),
        });

        setStatus("Creating window…");

        slint.setCanvasId("canvas");
        const appWindow = new ui.MainWindow();

        appWindow.PrinterState.ink_levels = [
            { color: "#00ffff", level: 0.3 },
            { color: "#ff00ff", level: 0.8 },
            { color: "#ffff00", level: 0.6 },
            { color: "#000000", level: 0.9 },
        ];

        const printerQueue = new slint.ArrayModel(
            Array.from(appWindow.PrinterQueue.printer_queue),
        );
        appWindow.PrinterQueue.printer_queue = printerQueue;

        appWindow.PrinterQueue.start_job = (title) => {
            const now = new Date();
            const pad = (n) => String(n).padStart(2, "0");
            const date =
                `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())} ` +
                `${pad(now.getDate())}/${pad(now.getMonth() + 1)}/${now.getFullYear()}`;

            printerQueue.push({
                status: "waiting",
                progress: 0,
                title,
                owner: "user@example.com",
                pages: 1,
                size: "100kB",
                submission_date: date,
            });
        };

        appWindow.PrinterQueue.cancel_job = (index) => {
            printerQueue.remove(index, 1);
        };

        const progressTimer = setInterval(() => {
            if (printerQueue.length > 0) {
                const top = printerQueue.rowData(0);
                top.progress += 1;
                if (top.progress > 100) {
                    printerQueue.remove(0, 1);
                } else {
                    top.status = "printing";
                    printerQueue.setRowData(0, top);
                }
            }
        }, 1000);

        setStatus("Running.");

        await appWindow.run();
        clearInterval(progressTimer);
        setStatus("Event loop ended.");
    } catch (err) {
        console.error(err);
        setStatus(`Error: ${err.message ?? err}`, true);
    }
}

main();
