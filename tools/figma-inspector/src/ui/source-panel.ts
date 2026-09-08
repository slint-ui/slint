// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

type SourceTheme = "light-slint" | "dark-slint";

export type SourcePanelOptions = {
    createWorker: () => Worker;
    reportClipboardResult: (success: boolean) => void;
    canHighlight: () => boolean;
    getRevision: () => number;
    onHighlightAccepted?: (revision: number) => void;
};

// Bound the text and resulting DOM without changing the raw source used by Copy.
function sourceForDisplay(source: string): string {
    const maxCharacters = 100_000;
    const maxLines = 2_000;
    const maxLineLength = 2_000;
    const lines: string[] = [];
    let offset = 0;
    let characters = 0;
    let shortened = false;
    while (
        offset < source.length &&
        lines.length < maxLines &&
        characters < maxCharacters
    ) {
        const end = source.indexOf("\n", offset);
        const stop = end < 0 ? source.length : end;
        let line = source.slice(offset, Math.min(stop, offset + maxLineLength));
        if (stop - offset > maxLineLength) {
            line = `${line.slice(0, maxLineLength - 250)} … [line shortened] … ${source.slice(stop - 200, stop)}`;
            shortened = true;
        }
        lines.push(line);
        characters += line.length + 1;
        offset = end < 0 ? source.length : end + 1;
    }
    if (offset < source.length) shortened = true;
    if (shortened)
        lines.push(
            "// Code view shortened for responsiveness. Copy Slint includes the complete source.",
        );
    else if (source.endsWith("\n")) lines.push("");
    return lines.join("\n");
}

function writeTextToClipboard(value: string): boolean {
    const previousActive = document.activeElement;
    const textArea = document.createElement("textarea");
    textArea.value = value;
    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    let copySuccessful = false;
    try {
        const successful = document.execCommand("copy");
        if (!successful) throw new Error("Copy command failed");
        copySuccessful = true;
    } catch (error: unknown) {
        const errorMessage =
            error instanceof Error ? error.message : String(error);
        console.error(`Failed to copy text: ${errorMessage}`);
    } finally {
        textArea.remove();
        if (previousActive && previousActive instanceof HTMLElement)
            previousActive.focus();
    }
    return copySuccessful;
}

export class SourcePanelController {
    private readonly sourceView: HTMLElement;
    private readonly options: SourcePanelOptions;
    private source = "";
    private theme: SourceTheme = "light-slint";
    private worker: Worker | undefined;
    private request = 0;
    private highlighting = false;
    private disposed = false;

    public constructor(sourceView: HTMLElement, options: SourcePanelOptions) {
        this.sourceView = sourceView;
        this.options = options;
    }

    public setSource(source: string): void {
        this.source = source;
        this.invalidate();
        if (source !== "" && this.options.canHighlight()) this.highlight();
    }

    public setTheme(theme: SourceTheme): void {
        this.theme = theme;
        if (this.source !== "" && this.options.canHighlight()) this.highlight();
    }

    public clear(): void {
        this.source = "";
        this.invalidate();
        this.sourceView.textContent = "";
    }

    public copy(text: string, button?: HTMLButtonElement): void {
        if (text === "") return;
        const success = writeTextToClipboard(text);
        this.options.reportClipboardResult(success);
        if (success && button !== undefined) {
            const label = button.textContent;
            button.textContent = "Copied";
            setTimeout(() => {
                button.textContent = label;
            }, 1000);
        }
    }

    public dispose(): void {
        this.disposed = true;
        this.request += 1;
        this.worker?.terminate();
        this.worker = undefined;
        this.highlighting = false;
    }

    private invalidate(): void {
        this.request += 1;
        if (this.highlighting) {
            this.worker?.terminate();
            this.worker = undefined;
        }
        this.highlighting = false;
        this.sourceView.setAttribute("aria-busy", "false");
    }

    private highlight(): void {
        this.invalidate();
        const request = this.request;
        const revision = this.options.getRevision();
        const source = sourceForDisplay(this.source);
        const theme = this.theme;
        const isCurrent = (): boolean =>
            !this.disposed &&
            request === this.request &&
            revision === this.options.getRevision() &&
            this.source !== "" &&
            this.theme === theme &&
            this.options.canHighlight();
        const fallback = (): void => {
            if (!isCurrent()) return;
            this.invalidate();
            this.sourceView.textContent =
                "Syntax highlighting unavailable. Copy Slint is still available.";
            this.sourceView.hidden = false;
        };
        try {
            if (!this.worker) this.worker = this.options.createWorker();
            this.highlighting = true;
            this.sourceView.setAttribute("aria-busy", "true");
            this.worker.onmessage = (event: MessageEvent<unknown>) => {
                if (!isCurrent()) return;
                const response = event.data;
                if (
                    !response ||
                    typeof response !== "object" ||
                    !("id" in response) ||
                    response.id !== request
                )
                    return;
                if (
                    !("html" in response) ||
                    typeof response.html !== "string"
                ) {
                    fallback();
                    return;
                }
                this.sourceView.innerHTML = response.html;
                this.sourceView.hidden = false;
                this.options.onHighlightAccepted?.(revision);
                this.highlighting = false;
                this.sourceView.setAttribute("aria-busy", "false");
            };
            this.worker.onerror = (event) => {
                event.preventDefault();
                fallback();
            };
            this.worker.onmessageerror = fallback;
            this.worker.postMessage({ id: request, source, theme });
        } catch {
            fallback();
        }
    }
}
