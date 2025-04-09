// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { dispatchTS } from "./bolt-utils.js";

export async function writeTextToClipboard(str: string) {
    const prevActive = document.activeElement;
    const textArea = document.createElement("textarea");

    textArea.value = str;

    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";

    document.body.appendChild(textArea);

    textArea.focus();
    textArea.select();

    try {
        const successful = document.execCommand("copy");
        if (!successful) {
            throw new Error("Copy command failed");
        }
    } catch (e: unknown) {
        const errorMessage = e instanceof Error ? e.message : String(e);
        throw new Error("Failed to copy text: " + errorMessage);
    } finally {
        textArea.remove();
        if (prevActive && prevActive instanceof HTMLElement) {
            prevActive.focus();
        }
    }
}

// Modify these functions to accept dispatchTS
interface DispatchTSFunction {
    (action: string, payload: { result: boolean }): void;
}

type CopyToClipboardFunction = (slintProperties: string) => Promise<void>;

export const getCopyToClipboard =
    (dispatchTS: DispatchTSFunction): CopyToClipboardFunction =>
    async (slintProperties: string) => {
        try {
            await writeTextToClipboard(slintProperties);
            dispatchTS("copyToClipboard", {
                result: true,
            });
        } catch (error) {
            dispatchTS("copyToClipboard", {
                result: false,
            });
        }
    };

type ExportAllFunction = () => void;

export const getExportAll =
    (dispatchTS: DispatchTSFunction): ExportAllFunction =>
    () => {
        dispatchTS("exportAll", {
            result: true,
        });
    };
