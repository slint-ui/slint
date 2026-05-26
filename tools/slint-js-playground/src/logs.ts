// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export type LogLevel = "log" | "warn" | "error";

/** Mirrors the iframe's console + uncaught errors into a div the user can read. */
export class LogPanel {
    #el: HTMLPreElement;

    constructor(el: HTMLPreElement) {
        this.#el = el;
    }

    clear(): void {
        this.#el.innerHTML = "";
    }

    append(level: LogLevel, text: string): void {
        const wasAtBottom =
            this.#el.scrollHeight - this.#el.scrollTop - this.#el.clientHeight <
            20;

        const entry = document.createElement("div");
        entry.className = `log-entry ${level}`;

        const tag = document.createElement("span");
        tag.className = "log-level";
        tag.textContent = level;
        entry.appendChild(tag);

        entry.appendChild(document.createTextNode(text));
        this.#el.appendChild(entry);

        if (wasAtBottom) {
            this.#el.scrollTop = this.#el.scrollHeight;
        }
    }
}
