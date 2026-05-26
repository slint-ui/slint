// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { DemoFiles } from "./files";
import { MAIN_JS, absoluteUrlFor } from "./files";
import type { LogLevel } from "./logs";

type LogCallback = (level: LogLevel, text: string) => void;
type ClearCallback = () => void;

/**
 * Each reload recreates the iframe — necessary because the slint winit event
 * loop on wasm cannot be re-entered within a single page session (calling
 * `run_event_loop` a second time throws).
 */
export class PreviewController {
    #iframe: HTMLIFrameElement;
    #onStatus: (text: string, isError: boolean) => void;
    #onClearLogs: ClearCallback;
    #onLog: LogCallback;
    #pendingRun: ReturnType<typeof setTimeout> | null = null;
    #current: DemoFiles | null = null;

    constructor(
        iframe: HTMLIFrameElement,
        onStatus: (text: string, isError: boolean) => void,
        onClearLogs: ClearCallback,
        onLog: LogCallback,
    ) {
        this.#iframe = iframe;
        this.#onStatus = onStatus;
        this.#onClearLogs = onClearLogs;
        this.#onLog = onLog;

        window.addEventListener("message", (event) => {
            const data = event.data as {
                type?: string;
                message?: string;
                level?: LogLevel;
                text?: string;
            };
            if (!data || typeof data.type !== "string") return;
            if (data.type === "ready" && this.#current) {
                this.#sendRunMessage(this.#current);
            } else if (data.type === "error") {
                this.#onStatus(`Error: ${data.message ?? "unknown"}`, true);
            } else if (data.type === "running") {
                this.#onStatus("Running.", false);
            } else if (
                data.type === "log" &&
                data.level !== undefined &&
                data.text !== undefined
            ) {
                this.#onLog(data.level, data.text);
            }
        });
    }

    /** Schedule a re-run, debounced by `delayMs`. */
    scheduleRun(current: DemoFiles, delayMs = 600): void {
        if (this.#pendingRun !== null) clearTimeout(this.#pendingRun);
        this.#pendingRun = setTimeout(() => {
            this.#pendingRun = null;
            this.runNow(current);
        }, delayMs);
    }

    /** Reload the iframe (the only way to restart the winit loop) and re-run. */
    runNow(current: DemoFiles): void {
        this.#current = current;
        this.#onClearLogs();
        this.#onStatus("Reloading…", false);
        // Recreate the iframe so the winit event loop starts fresh.
        const fresh = document.createElement("iframe");
        fresh.id = this.#iframe.id;
        fresh.className = this.#iframe.className;
        fresh.title = this.#iframe.title;
        fresh.src = "./preview.html";
        this.#iframe.replaceWith(fresh);
        this.#iframe = fresh;
        // The iframe will postMessage "ready" once preview-runtime.ts loads.
    }

    #sendRunMessage(current: DemoFiles): void {
        const { demo, files } = current;
        const mainSlint = demo.slintFiles[0];
        const slintFiles: Record<string, string> = {};
        for (const f of files.values()) {
            if (f.language === "slint") {
                slintFiles[absoluteUrlFor(demo.baseDir, f.relativePath)] = f.content;
            }
        }
        const mainJs = files.get(MAIN_JS)?.content ?? "";
        this.#iframe.contentWindow?.postMessage(
            {
                type: "run",
                userJs: mainJs,
                slintFiles,
                mainSlint,
                mainUrl: absoluteUrlFor(demo.baseDir, mainSlint),
                canvasWidth: demo.preferredWidth,
                canvasHeight: demo.preferredHeight,
            },
            "*",
        );
    }
}
