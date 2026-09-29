// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import type { UiToPluginMessage } from "../src/protocol";
import { mountPreview, type Preview } from "./browser-harness";

type PointerCapture = {
    readonly pointerId: () => number | undefined;
};

function resizeHandle(
    preview: Preview,
    direction: "right" | "bottom" | "bottomRight",
): HTMLElement {
    return preview.element(`[data-direction="${direction}"]`);
}

function installPointerCapture(handle: HTMLElement): PointerCapture {
    let pointerId: number | undefined;
    handle.setPointerCapture = (value) => {
        pointerId = value;
    };
    handle.hasPointerCapture = (value) => pointerId === value;
    handle.releasePointerCapture = (value) => {
        if (pointerId === value) pointerId = undefined;
    };
    return { pointerId: () => pointerId };
}

function sendPointer(
    preview: Preview,
    handle: HTMLElement,
    type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
    options: PointerEventInit,
): void {
    const Pointer = (preview.win as Window & typeof globalThis).PointerEvent;
    handle.dispatchEvent(
        new Pointer(type, {
            bubbles: true,
            pointerId: 1,
            pointerType: "mouse",
            isPrimary: true,
            ...options,
        }),
    );
}

function resizeMessages(preview: Preview) {
    return preview.messages.filter(
        (
            message,
        ): message is Extract<UiToPluginMessage, { type: "resizeWindow" }> =>
            message.type === "resizeWindow",
    );
}

test.each([
    ["right", 80, 0, 720, 480],
    ["bottom", 0, 80, 640, 560],
    ["bottomRight", 80, 80, 720, 560],
] as const)(
    "%s resize handles update the expected dimensions",
    async (direction, deltaX, deltaY, width, height) => {
        const preview = await mountPreview(false);
        const handle = resizeHandle(preview, direction);
        const capture = installPointerCapture(handle);
        sendPointer(preview, handle, "pointerdown", {
            button: 0,
            buttons: 1,
            clientX: 640,
            clientY: 480,
        });
        sendPointer(preview, handle, "pointermove", {
            button: -1,
            buttons: 1,
            clientX: 640 + deltaX,
            clientY: 480 + deltaY,
        });
        sendPointer(preview, handle, "pointerup", {
            button: 0,
            buttons: 0,
            clientX: 640 + deltaX,
            clientY: 480 + deltaY,
        });

        await expect
            .poll(() => resizeMessages(preview).at(-1))
            .toMatchObject({ width, height });
        expect(handle.dataset.active).toBe("false");
        expect(capture.pointerId()).toBeUndefined();
    },
);

test("a move without the primary button ends a stale resize", async () => {
    const preview = await mountPreview(false);
    const right = resizeHandle(preview, "right");
    const rightCapture = installPointerCapture(right);
    sendPointer(preview, right, "pointerdown", {
        button: 0,
        buttons: 1,
        clientX: 640,
        clientY: 240,
    });
    sendPointer(preview, right, "pointermove", {
        button: -1,
        buttons: 1,
        clientX: 700,
        clientY: 240,
    });
    sendPointer(preview, right, "pointermove", {
        button: -1,
        buttons: 0,
        clientX: 500,
        clientY: 240,
    });

    await expect
        .poll(() => resizeMessages(preview).at(-1))
        .toMatchObject({ width: 700, height: 480 });
    expect(right.dataset.active).toBe("false");
    expect(rightCapture.pointerId()).toBeUndefined();

    const messageCount = resizeMessages(preview).length;
    sendPointer(preview, right, "pointermove", {
        button: -1,
        buttons: 0,
        clientX: 520,
        clientY: 240,
    });
    await new Promise((resolve) => preview.win.requestAnimationFrame(resolve));
    expect(resizeMessages(preview)).toHaveLength(messageCount);

    const bottom = resizeHandle(preview, "bottom");
    installPointerCapture(bottom);
    sendPointer(preview, bottom, "pointerdown", {
        button: 0,
        buttons: 1,
        clientX: 320,
        clientY: 480,
    });
    sendPointer(preview, bottom, "pointerup", {
        button: 0,
        buttons: 0,
        clientX: 320,
        clientY: 540,
    });
    await expect
        .poll(() => resizeMessages(preview).at(-1))
        .toMatchObject({ width: 640, height: 540 });
});

test("a new press replaces a stale resize session", async () => {
    const preview = await mountPreview(false);
    const right = resizeHandle(preview, "right");
    const rightCapture = installPointerCapture(right);
    sendPointer(preview, right, "pointerdown", {
        button: 0,
        buttons: 1,
        clientX: 640,
        clientY: 240,
    });

    const bottom = resizeHandle(preview, "bottom");
    installPointerCapture(bottom);
    sendPointer(preview, bottom, "pointerdown", {
        button: 0,
        buttons: 1,
        clientX: 320,
        clientY: 480,
    });
    expect(right.dataset.active).toBe("false");
    expect(rightCapture.pointerId()).toBeUndefined();
    sendPointer(preview, bottom, "pointerup", {
        button: 0,
        buttons: 0,
        clientX: 320,
        clientY: 540,
    });

    await expect
        .poll(() => resizeMessages(preview).at(-1))
        .toMatchObject({ width: 640, height: 540 });
});

test("resize handles have larger edge and corner hit targets", async () => {
    const preview = await mountPreview(false);
    expect(
        preview.win.getComputedStyle(resizeHandle(preview, "right")).width,
    ).toBe("12px");
    expect(
        preview.win.getComputedStyle(resizeHandle(preview, "bottom")).height,
    ).toBe("12px");
    const cornerStyle = preview.win.getComputedStyle(
        resizeHandle(preview, "bottomRight"),
    );
    expect([cornerStyle.width, cornerStyle.height]).toEqual(["24px", "24px"]);
});
