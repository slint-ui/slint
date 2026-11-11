// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { ExportPackage } from "../protocol";
import type { AssetPreview } from "../asset-transport";

export function prepareExportValidation(
    pkg: ExportPackage,
    materialize: (packed: AssetPreview) => {
        source: string;
        dispose: () => void;
    },
): { source: string; dispose: () => void } {
    const files = new Map(pkg.files.map((file) => [file.path, file]));
    const mime: Record<string, string> = {
        png: "image/png",
        jpg: "image/jpeg",
        jpeg: "image/jpeg",
        gif: "image/gif",
        svg: "image/svg+xml",
    };
    const assets: string[] = [];
    const indices = new Map<string, number>();
    const source: (string | number)[] = [];
    const tokens =
        /\/\/[^\r\n]*|\/\*[\s\S]*?\*\/|@image-url\("(?:\\.|[^"\\])*"\)|"(?:\\.|[^"\\])*"/gu;
    let copied = 0;
    for (const match of pkg.source.matchAll(tokens)) {
        if (!match[0].startsWith("@image-url(")) continue;
        const path = JSON.parse(match[0].slice(11, -1)) as string;
        if (path.startsWith("data:") || path.startsWith("blob:")) continue;
        const file = files.get(path);
        if (!file || file.encoding !== "base64")
            throw Error(`Missing export image: ${path}`);
        const type = mime[path.slice(path.lastIndexOf(".") + 1)];
        if (!type) throw Error(`Unsupported export image: ${path}`);
        let index = indices.get(path);
        if (index === undefined) {
            index = assets.length;
            indices.set(path, index);
            assets.push(file.data);
        }
        source.push(
            pkg.source.slice(copied, match.index),
            `@image-url("data:${type};base64,`,
            index,
            '")',
        );
        copied = match.index + match[0].length;
    }
    source.push(pkg.source.slice(copied));
    return materialize({ assetVersion: 1, source, assets });
}
