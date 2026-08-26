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
    | "MouseCursor"
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

/** The value a property of each type holds when nothing assigns to it. */
const defaultValues: Partial<Record<KnownType, string>> = {
    angle: "0deg",
    bool: "false",
    brush: "a transparent brush",
    callback: '""',
    color: "a transparent color",
    "data-transfer": "an empty data-transfer",
    duration: "0ms",
    easing: "linear",
    enum: "the first enum value",
    float: "0.0",
    image: "the empty image",
    int: "0",
    keys: "@keys()",
    length: "0px",
    MouseCursor: "default",
    percent: "0%",
    "physical-length": "0phx",
    Edges: "0px",
    Point: "(0px, 0px)",
    Size: "(0px, 0px)",
    "styled-text": '""',
    "relative-font-size": "0rem",
    string: '""',
    struct: "a struct with all default values",
};

// `link-data.json` keys the documentation of a type by the type's own name.
export function getTypeInfo(typeName: KnownType): TypeInfo {
    const baseType = typeName.replace(/[\[\]]/g, "") as KnownType;
    const defaultValue = defaultValues[baseType];
    if (defaultValue === undefined || !(baseType in linkMap)) {
        console.error("Unknown type: ", typeName);
        return {
            href: "",
            defaultValue: "<???>",
        };
    }
    return { href: linkMap[baseType].href, defaultValue };
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
