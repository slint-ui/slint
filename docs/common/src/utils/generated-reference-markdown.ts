// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * Loads compiled Markdown for generated enum/struct reference snippets inlined into SlintProperty.
 *
 * Kept out of `utils.ts` so Playwright and Node can import `linkMap` from `utils.ts` without
 * evaluating Vite-only `import.meta.glob` (which is undefined outside the Vite transform pipeline).
 */

type MarkdownDocModule = { compiledContent: () => string };

/** Vite must see static glob patterns; runtime `import(\`...${name}.md\`)` fails SSR ("Unknown variable dynamic import"). */
const enumMarkdownLoaders = import.meta.glob<MarkdownDocModule>(
    "../../../astro/src/content/docs/generated/reference/enums/*.md",
);

const structMarkdownLoaders = import.meta.glob<MarkdownDocModule>(
    "../../../astro/src/content/docs/generated/reference/structs/*.md",
);

const stdWidgetMarkdownLoaders = import.meta.glob<MarkdownDocModule>(
    "../../../astro/src/content/collections/std-widgets/*.md",
);

function findGlobLoader(
    glob: Record<string, () => Promise<MarkdownDocModule>>,
    /** Unique path segment before the file name, e.g. `enums` or `structs`. */
    segment: string,
    baseName: string,
): (() => Promise<MarkdownDocModule>) | undefined {
    // Generated enum/struct partials are written with a `_` prefix so Astro
    // doesn't emit them as standalone pages; std-widgets snippets are not.
    const candidates = [
        `/${segment}/_${baseName}.md`,
        `/${segment}/${baseName}.md`,
    ];
    const keys = Object.keys(glob);
    for (const tail of candidates) {
        const hit = keys.find((key) =>
            key.replaceAll("\\", "/").endsWith(tail),
        );
        if (hit !== undefined) {
            return glob[hit];
        }
    }
    return undefined;
}

export async function getEnumContent(enumName: string | undefined) {
    if (!enumName) {
        return "";
    }
    const load = findGlobLoader(enumMarkdownLoaders, "enums", enumName);
    if (!load) {
        console.error(
            `No enum markdown for ${enumName} (run slint-doc-generator if docs/astro/generated enums are missing).`,
        );
        return "";
    }
    try {
        const module = await load();
        return module.compiledContent();
    } catch (error) {
        console.error(`Failed to load enum file for ${enumName}:`, error);
        return "";
    }
}

export async function getStructContent(
    structName: string | undefined,
): Promise<string> {
    if (structName === undefined) {
        return "";
    }
    const baseStruct = structName.replace(/[\[\]]/g, "");

    if (baseStruct === "Time" || baseStruct === "Date") {
        const load = findGlobLoader(
            stdWidgetMarkdownLoaders,
            "std-widgets",
            baseStruct,
        );
        if (!load) {
            console.error(`No std-widgets markdown for ${baseStruct}.`);
            return "";
        }
        try {
            const module = await load();
            return module.compiledContent();
        } catch (error) {
            console.error(
                `Failed to load std-widgets doc for ${baseStruct}:`,
                error,
            );
            return "";
        }
    }

    if (baseStruct) {
        const load = findGlobLoader(
            structMarkdownLoaders,
            "structs",
            baseStruct,
        );
        if (!load) {
            console.error(
                `No struct markdown for ${baseStruct} (run slint-doc-generator if generated struct docs are missing).`,
            );
            return "";
        }
        try {
            const module = await load();
            return module.compiledContent();
        } catch (error) {
            console.error(
                `Failed to load struct file for ${baseStruct}:`,
                error,
            );
            return "";
        }
    }
    return "";
}
