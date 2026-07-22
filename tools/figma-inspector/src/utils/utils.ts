// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import JSZip from "jszip";

export function writeTextToClipboard(str: string): boolean {
    const prevActive = document.activeElement;
    const textArea = document.createElement("textarea");

    textArea.value = str;

    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";

    document.body.appendChild(textArea);

    textArea.focus();
    textArea.select();

    let copySuccessful = false;

    try {
        const successful = document.execCommand("copy");
        if (!successful) {
            throw new Error("Copy command failed");
        }
        copySuccessful = true;
    } catch (e: unknown) {
        const errorMessage = e instanceof Error ? e.message : String(e);
        console.error("Failed to copy text: " + errorMessage);
    } finally {
        textArea.remove();
        if (prevActive && prevActive instanceof HTMLElement) {
            prevActive.focus();
        }
    }
    return copySuccessful;
}

export async function downloadZipFile(
    files: Array<{ name: string; content: string }>,
) {
    try {
        if (!files || files.length === 0) {
            console.error("No files to zip!");
            return;
        }

        // Create a new JSZip instance directly (using the import)
        const zip = new JSZip();

        // Add each file to the zip with debug logging
        files.forEach((file) => {
            zip.file(file.name, file.content);
        });

        // Generate the zip
        const content = await zip.generateAsync({ type: "blob" });

        // Create download link
        const element = document.createElement("a");
        element.href = URL.createObjectURL(content);
        element.download = "figma-collections.zip";
        document.body.appendChild(element);
        element.click();
        document.body.removeChild(element);

        // Clean up
        URL.revokeObjectURL(element.href);
    } catch (error) {
        console.error("Error creating ZIP file:", error);

        // Fallback to individual downloads if ZIP creation fails
        alert("Couldn't create ZIP file. Downloading files individually...");
        files.forEach((file, index) => {
            setTimeout(() => {
                downloadFile(file.name, file.content);
            }, index * 100);
        });
    }
}

function downloadFile(filename: string, text: string) {
    const element = document.createElement("a");
    element.setAttribute(
        "href",
        "data:text/plain;charset=utf-8," + encodeURIComponent(text),
    );
    element.setAttribute("download", filename);
    element.style.display = "none";
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
}
