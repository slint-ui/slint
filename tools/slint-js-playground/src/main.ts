// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { EditorUi } from "./editor";
import { PreviewController } from "./preview";
import { loadDemoFiles, MAIN_JS, type DemoFiles } from "./files";
import { LogPanel } from "./logs";
import { DEMOS, DEFAULT_DEMO_ID, type Demo } from "./demos";

const LOGS_TAB_ID = "(logs)";

const statusEl = document.getElementById("status") as HTMLSpanElement;
function setStatus(text: string, isError = false) {
    statusEl.textContent = text;
    statusEl.classList.toggle("error", isError);
}

function populateDemoSelector(select: HTMLSelectElement, selectedId: string) {
    select.innerHTML = "";
    for (const demo of DEMOS) {
        const opt = document.createElement("option");
        opt.value = demo.id;
        opt.textContent = demo.label;
        if (demo.id === selectedId) opt.selected = true;
        select.appendChild(opt);
    }
}

async function main() {
    const initialDemo =
        DEMOS.find((d) => d.id === DEFAULT_DEMO_ID) ?? DEMOS[0];

    setStatus(`Loading ${initialDemo.label}…`);

    let current: DemoFiles = await loadDemoFiles(initialDemo);

    const logPanel = new LogPanel(
        document.getElementById("logs") as HTMLPreElement,
    );

    const editor = new EditorUi(
        current.files,
        {
            tabs: document.getElementById("tabs") as HTMLDivElement,
            editor: document.getElementById("editor") as HTMLDivElement,
        },
        () => {
            preview.scheduleRun(current);
        },
    );

    editor.addVirtualTab({
        id: LOGS_TAB_ID,
        label: "Logs",
        element: document.getElementById("logs") as HTMLElement,
    });

    const preview = new PreviewController(
        document.getElementById("preview") as HTMLIFrameElement,
        setStatus,
        () => logPanel.clear(),
        (level, text) => logPanel.append(level, text),
    );

    function openAllTabs(): void {
        for (const path of current.files.keys()) {
            if (path !== MAIN_JS) editor.openFile(path, false);
        }
        editor.openFile(MAIN_JS);
    }

    async function switchDemo(demo: Demo): Promise<void> {
        setStatus(`Loading ${demo.label}…`);
        try {
            current = await loadDemoFiles(demo);
        } catch (err) {
            setStatus(`Failed: ${(err as Error).message}`, true);
            return;
        }
        editor.reset(current.files);
        openAllTabs();
        setStatus("Starting preview…");
        preview.runNow(current);
    }

    const select = document.getElementById("demo-select") as HTMLSelectElement;
    populateDemoSelector(select, initialDemo.id);
    select.addEventListener("change", () => {
        const next = DEMOS.find((d) => d.id === select.value);
        if (next) void switchDemo(next);
    });

    openAllTabs();

    const reloadBtn = document.getElementById("reload-button");
    reloadBtn?.addEventListener("click", () => preview.runNow(current));

    window.addEventListener("keydown", (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === "s") {
            e.preventDefault();
            preview.runNow(current);
        }
    });

    logPanel.append(
        "log",
        `[playground] parent ready; ${current.files.size} files`,
    );

    setStatus("Starting preview…");
    preview.runNow(current);
}

main().catch((err) => {
    console.error(err);
    setStatus(`Failed to start: ${err.message ?? err}`, true);
});
