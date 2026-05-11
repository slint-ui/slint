// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import linkMapData from "../../../../internal/core-macros/link-data.json" with {
    type: "json",
};

export type LinkMapType = {
    [K: string]: {
        href: string;
    };
};

export const linkMap: Readonly<LinkMapType> = linkMapData;

type MarkdownDocModule = { compiledContent: () => string };

/** Vite must see static glob patterns; runtime `import(\`...${name}.md\`)` fails SSR ("Unknown variable dynamic import"). */
const enumMarkdownLoaders = import.meta.glob<MarkdownDocModule>(
    "../../../astro/src/content/docs/reference/generated/enums/*.md",
);

const structMarkdownLoaders = import.meta.glob<MarkdownDocModule>(
    "../../../astro/src/content/docs/reference/generated/structs/*.md",
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
    const tail = `/${segment}/${baseName}.md`;
    const hit = Object.keys(glob).find((key) => key.replaceAll("\\", "/").endsWith(tail));
    return hit !== undefined ? glob[hit] : undefined;
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
        const load = findGlobLoader(stdWidgetMarkdownLoaders, "std-widgets", baseStruct);
        if (!load) {
            console.error(`No std-widgets markdown for ${baseStruct}.`);
            return "";
        }
        try {
            const module = await load();
            return module.compiledContent();
        } catch (error) {
            console.error(`Failed to load std-widgets doc for ${baseStruct}:`, error);
            return "";
        }
    }

    if (baseStruct) {
        const load = findGlobLoader(structMarkdownLoaders, "structs", baseStruct);
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
            console.error(`Failed to load struct file for ${baseStruct}:`, error);
            return "";
        }
    }
    return "";
}

export type KnownType =
    | "angle"
    | "bool"
    | "brush"
    | "callback"
    | "color"
    | "data-transfer"
    | "duration"
    | "easing"
    | "enum"
    | "float"
    | "function"
    | "image"
    | "int"
    | "keys"
    | "length"
    | "percent"
    | "physical-length"
    | "Edges"
    | "Point"
    | "Size"
    | "styled-text"
    | "relative-font-size"
    | "string"
    | "struct";

export type PropertyVisibility = "private" | "in" | "out" | "in-out";

export interface TypeInfo {
    href: string;
    defaultValue: string;
}

export function getTypeInfo(typeName: KnownType): TypeInfo {
    const baseType = typeName.replace(/[\[\]]/g, "") as KnownType;
    switch (baseType) {
        case "angle":
            return {
                href: linkMap.angle.href,
                defaultValue: "0deg",
            };
        case "bool":
            return {
                href: linkMap.bool.href,
                defaultValue: "false",
            };
        case "brush":
            return {
                href: linkMap.brush.href,
                defaultValue: "a transparent brush",
            };
        case "color":
            return {
                href: linkMap.color.href,
                defaultValue: "a transparent color",
            };
        case "data-transfer":
            return {
                href: linkMap.data_transfer.href,
                defaultValue: "an empty data-transfer",
            };
        case "duration":
            return {
                href: linkMap.duration.href,
                defaultValue: "0ms",
            };
        case "easing":
            return {
                href: linkMap.easing.href,
                defaultValue: "linear",
            };
        case "enum":
            return {
                href: linkMap.EnumType.href,
                defaultValue: "the first enum value",
            };
        case "Edges":
            return {
                href: linkMap.Edges.href,
                defaultValue: "0px",
            };
        case "float":
            return {
                href: linkMap.float.href,
                defaultValue: "0.0",
            };
        case "image":
            return {
                href: linkMap.ImageType.href,
                defaultValue: "the empty image",
            };
        case "keys":
            return {
                href: linkMap["keys"].href,
                defaultValue: "@keys()",
            };
        case "int":
            return {
                href: linkMap.int.href,
                defaultValue: "0",
            };
        case "length":
            return {
                href: linkMap.length.href,
                defaultValue: "0px",
            };
        case "percent":
            return {
                href: linkMap.percent.href,
                defaultValue: "0%",
            };
        case "physical-length":
            return {
                href: linkMap.physicalLength.href,
                defaultValue: "0phx",
            };
        case "Point":
            return {
                href: linkMap.Point.href,
                defaultValue: "(0px, 0px)",
            };
        case "Size":
            return {
                href: linkMap.Size.href,
                defaultValue: "(0px, 0px)",
            };
        case "relative-font-size":
            return {
                href: linkMap.relativeFontSize.href,
                defaultValue: "0rem",
            };
        case "string":
            return {
                href: linkMap.StringType.href,
                defaultValue: '""',
            };
        case "styled-text":
            return {
                href: linkMap.styled_text.href,
                defaultValue: '""',
            };
        case "callback":
            return {
                href: linkMap.callback.href,
                defaultValue: '""',
            };
        case "struct":
            return {
                href: linkMap.StructType.href,
                defaultValue: "a struct with all default values",
            };
        default: {
            console.error("Unknown type: ", typeName);
            return {
                href: "",
                defaultValue: "<???>",
            };
        }
    }
}

export function extractLines(
    fileContent: string,
    start: number,
    end: number,
): string {
    return fileContent
        .split("\n")
        .slice(start - 1, end)
        .join("\n");
}

export function removeLeadingSpaces(input: string, spaces = 4): string {
    const lines = input.split("\n");
    const modifiedLines = lines.map((line) => {
        const leadingSpaces = line.match(/^ */)?.[0].length ?? 0;
        if (leadingSpaces >= spaces) {
            return line.slice(spaces);
        }
        return line;
    });
    return modifiedLines.join("\n");
}

export const trim = (str = "", ch?: string) => {
    let start = 0;
    let end = str.length || 0;
    while (start < end && str[start] === ch) {
        ++start;
    }
    while (end > start && str[end - 1] === ch) {
        --end;
    }
    return start > 0 || end < str.length ? str.substring(start, end) : str;
};
