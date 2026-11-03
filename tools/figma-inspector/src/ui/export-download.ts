// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import JSZip from "jszip";
import { isExportPackage, type ExportPackage } from "../protocol";

export async function exportZip(value: ExportPackage): Promise<Uint8Array> {
    if (!isExportPackage(value)) throw Error("Invalid export package");
    const zip = new JSZip();
    const date = new Date(1980, 0, 1);
    zip.file("main.slint", value.source, { date });
    zip.file("assets/", "", { dir: true, date });
    zip.file("fonts/", "", { dir: true, date });
    for (const file of value.files)
        zip.file(file.path, file.data, {
            base64: file.encoding === "base64",
            date,
            createFolders: false,
        });
    return zip.generateAsync({
        type: "uint8array",
        compression: "DEFLATE",
        compressionOptions: { level: 6 },
    });
}

export async function downloadExport(
    value: ExportPackage,
    name: string,
    isCurrent: () => boolean = () => true,
): Promise<void> {
    const bytes = await exportZip(value);
    if (!isCurrent()) return;
    const url = URL.createObjectURL(
        new Blob([bytes as Uint8Array<ArrayBuffer>], {
            type: "application/zip",
        }),
    );
    const link = document.createElement("a");
    link.href = url;
    link.download = `${name.replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-|-$/g, "") || "slint-export"}.zip`;
    document.body.append(link);
    link.click();
    link.remove();
    // Give the browser time to consume the download URL before releasing it.
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
