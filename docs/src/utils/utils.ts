// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

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

const KnownStructs = ["Point"];
export async function getStructContent(
    typeName: KnownType | undefined,
): Promise<string> {
    if (typeName === undefined) {
        return "";
    }
    if (KnownStructs.includes(typeName)) {
        if (typeName) {
            try {
                const module = await import(
                    `../content/collections/structs/${typeName}.md`
                );
                return module.compiledContent();
            } catch (error) {
                console.error(
                    `Failed to load enum file for ${typeName}:`,
                    error,
                );
                return "";
            }
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
    switch (typeName) {
        case "angle":
            return {
                href: linkMap.Types.href,
                defaultValue: "0deg",
            };
        case "bool":
            return {
                href: linkMap.Types.href,
                defaultValue: "false",
            };
        case "brush":
            return {
                href: linkMap.ColorsRef.href,
                defaultValue: "a transparent brush",
            };
        case "color":
            return {
                href: linkMap.ColorsRef.href,
                defaultValue: "a transparent color",
            };
        case "duration":
            return {
                href: linkMap.Types.href,
                defaultValue: "0ms",
            };
        case "easing":
            return {
                href: linkMap.Types.href,
                defaultValue: "linear",
            };
        case "enum":
            return {
                href: "", // No need to link here!
                defaultValue: "the first enum value",
            };
        case "float":
            return {
                href: linkMap.Types.href,
                defaultValue: "0.0",
            };
        case "image":
            return {
                href: linkMap.ImageType.href,
                defaultValue: "the empty image",
            };
        case "int":
            return {
                href: linkMap.Types.href,
                defaultValue: "0",
            };
        case "length":
            return {
                href: linkMap.Types.href,
                defaultValue: "0px",
            };
        case "percent":
            return {
                href: linkMap.Types.href,
                defaultValue: "0%",
            };
        case "physical-length":
            return {
                href: linkMap.Types.href,
                defaultValue: "0phx",
            };
        case "Point":
            return {
                href: linkMap.Types.href,
                defaultValue: "(0px, 0px)",
            };
        case "relative-font-size":
            return {
                href: linkMap.Types.href,
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

export function removeLeadingSpaces(input: string, spaces: number = 4): string {
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


type LinkMapType = {
    [K: string]: {
        href: string;
    };
};

import linkMapData from "./link-data.json" assert { type: "json" };
export const linkMap: Readonly<LinkMapType> = linkMapData;
