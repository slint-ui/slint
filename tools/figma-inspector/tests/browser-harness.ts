// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { afterEach, expect } from "vitest";
import { page, server } from "vitest/browser";
import type { PluginToUiMessage, UiToPluginMessage } from "../src/protocol";

const cleanups: (() => void)[] = [];
afterEach(() => {
    for (const dispose of cleanups.splice(0)) dispose();
});
export const readFixture = (path: string) =>
    server.commands.readFile(path, "utf8");

/** Mount the exact built UI and complete its Figma message handshake. */
export async function mountPreview(production = true) {
    const iframe = document.createElement("iframe");
    iframe.style.cssText = "width:640px;height:480px;border:0;display:block";
    const messages: UiToPluginMessage[] = [];
    const receive = (event: MessageEvent) => {
        if (event.source === iframe.contentWindow && event.data?.pluginMessage)
            messages.push(event.data.pluginMessage);
    };
    window.addEventListener("message", receive);
    cleanups.push(() => {
        window.removeEventListener("message", receive);
        iframe.remove();
    });
    const loaded = new Promise<void>((resolve) =>
        iframe.addEventListener("load", () => resolve(), { once: true }),
    );
    iframe.src = production ? "/production.html" : "/browser.html";
    document.body.append(iframe);
    await loaded;
    const doc = iframe.contentDocument;
    const win = iframe.contentWindow;
    if (!doc || !win) throw Error("Preview document did not load");
    await expect
        .poll(() => messages.some((m) => m.type === "ui-ready"))
        .toBe(true);
    function element<T extends HTMLElement = HTMLElement>(selector: string): T {
        const target = doc?.querySelector<T>(selector);
        if (!target) throw Error(`Missing preview element ${selector}`);
        return target;
    }
    function send(message: PluginToUiMessage) {
        win?.postMessage({ pluginMessage: message }, "*");
    }
    async function ready(revision: number) {
        await expect
            .poll(() => [
                element("#status").dataset.state,
                element("#status").dataset.revision,
            ])
            .toEqual(["ready", String(revision)]);
    }
    return { iframe, doc, win, messages, element, send, ready };
}
export type Preview = Awaited<ReturnType<typeof mountPreview>>;

/** Native browser screenshot of the composited canvas; WebGL buffers may be discarded. */
export async function canvasPixels(preview: Preview) {
    const canvas = preview.element<HTMLCanvasElement>("#preview-canvas");
    const width = canvas.getBoundingClientRect().width,
        height = canvas.getBoundingClientRect().height;
    const style = preview.doc.createElement("style");
    style.textContent = `html,body {margin:0!important;overflow:hidden!important;background:white!important} body * {visibility:hidden!important} #preview-canvas {visibility:visible!important;display:block!important;position:fixed!important;left:0!important;top:0!important;width:${width}px!important;height:${height}px!important;transform:none!important;border-radius:0!important;background:white!important}`;
    preview.doc.head.append(style);
    const previous = preview.iframe.style.cssText;
    preview.iframe.style.width = `${width}px`;
    preview.iframe.style.height = `${height}px`;
    try {
        const base64 = await page
            .elementLocator(preview.iframe)
            .screenshot({ save: false });
        return await decodePng(base64);
    } finally {
        style.remove();
        preview.iframe.style.cssText = previous;
    }
}

export async function decodePng(base64: string) {
    const image = new Image();
    image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext("2d");
    if (!context) throw Error("Missing 2D context");
    context.fillStyle = "white";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0);
    return {
        width: canvas.width,
        height: canvas.height,
        data: context.getImageData(0, 0, canvas.width, canvas.height).data,
    };
}
