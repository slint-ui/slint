// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * The in-memory file system that backs the editor tabs and the preview's
 * `fileLoader`. Keyed by `relativePath` (e.g. `"common.slint"` or
 * `"pages/home_page.slint"`).
 *
 * The Slint compiler sees the *absolute* URLs returned by
 * {@link absoluteUrlFor}, so that `@image-url("./images/...")` resolves to
 * the real `/@fs/` URL that vite serves the binary assets from.
 */

import type { Demo } from "./demos";

export interface PlaygroundFile {
    relativePath: string;
    language: "slint" | "javascript";
    /** Mutable: edited content. */
    content: string;
    /** Set to true when the user has edited this file. */
    dirty: boolean;
}

export interface DemoFiles {
    demo: Demo;
    files: FileMap;
}

export type FileMap = Map<string, PlaygroundFile>;

export const MAIN_JS = "main.js";

/** Build the absolute `/@fs/` URL the Slint compiler sees for a file. */
export function absoluteUrlFor(baseDir: string, relativePath: string): string {
    return `${window.location.origin}/@fs${baseDir}/${relativePath}`;
}

async function fetchText(url: string): Promise<string> {
    const r = await fetch(url);
    if (!r.ok) {
        throw new Error(`fetch ${url}: ${r.status}`);
    }
    return await r.text();
}

/** Fetch a demo's .slint sources and seed {@link MAIN_JS}. */
export async function loadDemoFiles(demo: Demo): Promise<DemoFiles> {
    const files: FileMap = new Map();

    files.set(MAIN_JS, {
        relativePath: MAIN_JS,
        language: "javascript",
        content: demo.mainJs,
        dirty: false,
    });

    await Promise.all(
        demo.slintFiles.map(async (rel) => {
            const content = await fetchText(absoluteUrlFor(demo.baseDir, rel));
            files.set(rel, {
                relativePath: rel,
                language: "slint",
                content,
                dirty: false,
            });
        }),
    );

    return { demo, files };
}
