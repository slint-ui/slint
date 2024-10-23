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
                href: "/guide/types/",
                defaultValue: "0deg",
            };
        case "bool":
            return {
                href: "/guide/types/",
                defaultValue: "false",
            };
        case "brush":
            return {
                href: "/guide/types/#colors-and-brushes",
                defaultValue: "a transparent brush",
            };
        case "color":
            return {
                href: "/guide/types/#colors-and-brushes",
                defaultValue: "a transparent color",
            };
        case "duration":
            return {
                href: "/guide/types/",
                defaultValue: "0ms",
            };
        case "easing":
            return {
                href: "/guide/types/",
                defaultValue: "linear",
            };
        case "enum":
            return {
                href: "", // No need to link here!
                defaultValue: "the first enum value",
            };
        case "float":
            return {
                href: "/guide/types/",
                defaultValue: "0.0",
            };
        case "image":
            return {
                href: "/guide/types/#images",
                defaultValue: "the empty image",
            };
        case "int":
            return {
                href: "/guide/types/",
                defaultValue: "0",
            };
        case "length":
            return {
                href: "/guide/types/",
                defaultValue: "0px",
            };
        case "percent":
            return {
                href: "/guide/types/",
                defaultValue: "0%",
            };
        case "physical-length":
            return {
                href: "/guide/types/",
                defaultValue: "0phx",
            };
        case "relative-font-size":
            return {
                href: "/guide/types/",
                defaultValue: "0rem",
            };
        case "string":
            return {
                href: "/guide/types/#string",
                defaultValue: '""',
            };
        case "struct":
            return {
                href: "/guide/types/#structs",
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
