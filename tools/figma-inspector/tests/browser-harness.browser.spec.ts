// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test, vi } from "vitest";
import { page, server } from "vitest/browser";
import { canvasPixels, mountPreview } from "./browser-harness";

test("canvas screenshots wait for presentation after resizing the iframe", async () => {
    const preview = await mountPreview();
    const png = await server.commands.readFile(
        "fixtures/authored/square.png",
        "base64",
    );
    const frames: FrameRequestCallback[] = [];
    const locator = page.elementLocator(preview.iframe);
    const screenshot = vi.spyOn(locator, "screenshot").mockResolvedValue(png);
    vi.spyOn(page, "elementLocator").mockReturnValue(locator);
    vi.spyOn(preview.win, "requestAnimationFrame").mockImplementation(
        (callback) => {
            frames.push(callback);
            return frames.length;
        },
    );
    try {
        const pixels = canvasPixels(preview);
        expect(screenshot).not.toHaveBeenCalled();
        const firstFrame = frames.shift();
        if (!firstFrame) throw Error("Expected a presentation frame");
        firstFrame(0);
        expect(screenshot).not.toHaveBeenCalled();
        const presentedFrame = frames.shift();
        if (!presentedFrame) throw Error("Expected a presented frame");
        presentedFrame(16);
        expect((await pixels).width).toBe(24);
        expect(screenshot).toHaveBeenCalledOnce();
    } finally {
        vi.restoreAllMocks();
    }
});
