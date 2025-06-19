// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import linkMapData from "../../../../internal/core-macros/link-data.json" with {
    type: "json",
};

type LinkMapType = {
    [K: string]: {
        href: string;
    };
};

export const linkMap: Readonly<LinkMapType> = linkMapData;

export async function getEnumContent(enumName: string | undefined) {
    if (enumName) {
        try {
            const module = await import(
                `../content/collections/enums/${enumName}.md`
            );
            return module.compiledContent();
        } catch (error) {
            console.error(`Failed to load enum file for ${enumName}:`, error);
            return "";
        }
    }
    return "";
}

export async function getStructContent(
    structName: string | undefined,
): Promise<string> {
    if (structName === undefined) {
        return "";
    }
    const baseStruct = structName.replace(/[\[\]]/g, "");

    if (baseStruct === "Time" || baseStruct === "Date") {
        try {
            const module = await import(
                `../content/collections/std-widgets/${baseStruct}.md`
            );
            return module.compiledContent();
        } catch (error) {
            console.error(`Failed to load enum file for ${baseStruct}:`, error);
            return "";
        }
    }

    if (baseStruct) {
        try {
            const module = await import(
                `../content/collections/structs/${baseStruct}.md`
            );
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

export type KnownType =
    | "angle"
    | "bool"
    | "brush"
    | "color"
    | "duration"
    | "easing"
    | "enum"
    | "float"
    | "image"
    | "int"
    | "length"
    | "percent"
    | "physical-length"
    | "Point"
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
                href: "", // No need to link here!
                defaultValue: "the first enum value",
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
                href: linkMap.StructType.href,
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
