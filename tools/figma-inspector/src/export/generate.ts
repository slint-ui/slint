// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex } from "@noble/hashes/utils.js";
import { warningSummaries } from "../plugin/snapshot";
import { identifier, nameAllocator } from "../preview/component-names";
import { convertSnapshot } from "../preview/converter";
import type {
    FigmaSnapshot,
    SnapshotNode,
    Diagnostic,
} from "../plugin/snapshot";
import type { ExportFile, ExportPackage } from "../protocol";

/** Rewrite only image-url expressions, never string contents (including labels
 * that happen to contain Slint snippets). No structure is recovered from code. */
export function externalizeImages(source: string): {
    source: string;
    files: ExportFile[];
} {
    const files = new Map<string, ExportFile>();
    let output = "",
        i = 0;
    while (i < source.length) {
        if (source[i] === '"') {
            const start = i++;
            while (i < source.length) {
                if (source[i++] === "\\") i++;
                else if (source[i - 1] === '"') break;
            }
            output += source.slice(start, i);
            continue;
        }
        const match = source
            .slice(i)
            .match(
                /^@image-url\("data:image\/(png|jpeg|gif|svg\+xml);base64,([A-Za-z0-9+/=]+)"\)/,
            );
        if (match) {
            const extension = {
                png: "png",
                jpeg: "jpg",
                gif: "gif",
                "svg+xml": "svg",
            }[match[1]];
            const digest = bytesToHex(
                sha256(new TextEncoder().encode(match[0])),
            ).slice(0, 16);
            const path = `assets/image-${digest}.${extension}`;
            files.set(path, { path, data: match[2], encoding: "base64" });
            output += `@image-url("${path}")`;
            i += match[0].length;
        } else output += source[i++];
    }
    return {
        source: output,
        files: [...files.values()].sort((a, b) =>
            a.path < b.path ? -1 : a.path > b.path ? 1 : 0,
        ),
    };
}

export function generateExport(
    snapshot: FigmaSnapshot,
    warnings: readonly Diagnostic[] = [],
): ExportPackage {
    const converted = convertSnapshot(snapshot, { target: "export" });
    if (!converted.ok)
        throw Error(converted.diagnostics.map((d) => d.message).join("\n"));
    const fonts = new Map<string, { family: string; style: string }>();
    function add(family: string, style: string) {
        fonts.set(JSON.stringify([family, style]), { family, style });
    }
    function visit(node: SnapshotNode) {
        if (node.kind === "text") {
            add(node.fontFamily, node.fontStyle);
            for (const run of node.runs ?? []) {
                if (run.bold || run.italic)
                    add(
                        node.fontFamily,
                        `${run.bold ? "Bold" : ""}${run.italic ? "Italic" : ""}`,
                    );
            }
        }
        if ("children" in node) node.children.forEach(visit);
    }
    visit(snapshot.root);
    for (const definition of snapshot.components?.definitions ?? [])
        for (const variant of definition.variants) visit(variant.root);
    const allocate = nameAllocator();
    const requiredFonts = [...fonts.values()]
        .sort((a, b) =>
            JSON.stringify(a) < JSON.stringify(b)
                ? -1
                : JSON.stringify(a) > JSON.stringify(b)
                  ? 1
                  : 0,
        )
        .map((font) => ({
            ...font,
            path: `fonts/${allocate(identifier(`${font.family}-${font.style}`))}.ttf`,
        }));
    const external = externalizeImages(converted.source);
    const imports = requiredFonts.map((font) => `// import "${font.path}";`);
    return {
        source: imports.length
            ? `${imports.join("\n")}\n\n${external.source}`
            : external.source,
        files: [
            ...external.files,
            {
                path: "fonts/README.txt",
                encoding: "utf8",
                data: requiredFonts.length
                    ? "Font imports at the top of main.slint are commented out so the export can run in Slint development tools without the font files. Font binaries are not included; fallback fonts may change the text appearance and layout.\nAdd the following real font files, then uncomment their import lines in main.slint by removing the leading // to enable them.\nNames below are expected filenames, not original Figma file paths. For OTF/TTC fonts, change the import extension to match.\n\n" +
                      requiredFonts
                          .map(
                              (font) =>
                                  `${font.path}: ${font.family} / ${font.style}`,
                          )
                          .join("\n") +
                      "\n"
                    : "This export requires no font files.\n",
            },
            {
                path: "README.txt",
                encoding: "utf8",
                data:
                    "Open main.slint in your Slint project. Keep assets/ and fonts/ beside it.\nFont imports are commented out so you can run the export without the font files. See fonts/README.txt for the font checklist and instructions to enable the imports after adding the real fonts.\nText remains native; images are stored in assets/.\n\n" +
                    warningSummaries([...warnings, ...converted.warnings])
                        .map((d) => `${d.code}: ${d.message}`)
                        .join("\n") +
                    "\n",
            },
        ],
    };
}
