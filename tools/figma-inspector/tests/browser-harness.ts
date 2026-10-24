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
export async function mountPreview(production = true, canvasLayout = false) {
    const iframe = document.createElement("iframe");
    iframe.style.cssText = canvasLayout
        ? "width:1280px;height:1280px;border:0;display:block"
        : "width:640px;height:480px;border:0;display:block";
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
    iframe.src = production ? "/production.html" : "/ui.html";
    if (canvasLayout) document.body.prepend(iframe);
    else document.body.append(iframe);
    await loaded;
    const doc = iframe.contentDocument;
    const win = iframe.contentWindow;
    if (!doc || !win) throw Error("Preview document did not load");
    if (canvasLayout) {
        const style = doc.createElement("style");
        // Align the canvas to device pixels before Slint creates its WebGL surface.
        style.textContent =
            "#preview-canvas {position:fixed!important;left:0!important;top:0!important;transform:none!important;border-radius:0!important;background:white!important;outline:none!important;z-index:2147483647!important}";
        doc.head.append(style);
        doc.querySelector("#preview-canvas")?.setAttribute(
            "data-testid",
            "preview-canvas",
        );
    }
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

export const mountCanvasPreview = (production = true) =>
    mountPreview(production, true);

function canvasLocator(preview: Preview) {
    return page
        .frameLocator(page.elementLocator(preview.iframe))
        .getByTestId("preview-canvas");
}

const canvasTimeout = 10000;

/** Native browser screenshot of the composited canvas; WebGL buffers may be discarded. */
export async function canvasPixels(preview: Preview, timeout = canvasTimeout) {
    return decodePng(
        await canvasLocator(preview).screenshot({ save: false, timeout }),
    );
}

export type CanvasPixels = Awaited<ReturnType<typeof canvasPixels>>;

export async function expectCanvasPixels(
    preview: Preview,
    check: (pixels: CanvasPixels) => void,
    { timeout = canvasTimeout }: { timeout?: number } = {},
): Promise<CanvasPixels> {
    let actual: CanvasPixels | undefined;
    const deadline = performance.now() + timeout;
    try {
        await expect
            .poll(
                async ({ signal }) => {
                    const pixels = await canvasPixels(
                        preview,
                        Math.max(1, deadline - performance.now()),
                    );
                    if (signal.aborted) return false;
                    check(pixels);
                    actual = pixels;
                    return true;
                },
                { timeout },
            )
            .toBe(true);
    } catch (error) {
        const path = await canvasLocator(preview)
            .screenshot({ timeout })
            .catch(() => undefined);
        if (error instanceof Error && path)
            error.message += `\nFinal canvas screenshot: ${path}`;
        throw error;
    }
    if (!actual) throw Error("Missing canvas screenshot");
    return actual;
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
