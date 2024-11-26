// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { h } from "@astrojs/starlight/expressive-code/hast";

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
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0deg",
            };
        case "bool":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "false",
            };
        case "brush":
            return {
                href: "/master/docs/slint/guide/language/types/#colors-and-brushes",
                defaultValue: "a transparent brush",
            };
        case "color":
            return {
                href: "/master/docs/slint/guide/language/types/#colors-and-brushes",
                defaultValue: "a transparent color",
            };
        case "duration":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0ms",
            };
        case "easing":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "linear",
            };
        case "enum":
            return {
                href: "", // No need to link here!
                defaultValue: "the first enum value",
            };
        case "float":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0.0",
            };
        case "image":
            return {
                href: "/master/docs/slint/guide/language/types/#images",
                defaultValue: "the empty image",
            };
        case "int":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0",
            };
        case "length":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0px",
            };
        case "percent":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0%",
            };
        case "physical-length":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0phx",
            };
        case "Point":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "(0px, 0px)",
            };
        case "relative-font-size":
            return {
                href: "/master/docs/slint/guide/language/types/",
                defaultValue: "0rem",
            };
        case "string":
            return {
                href: "/master/docs/slint/guide/language/types/#string",
                defaultValue: '""',
            };
        case "struct":
            return {
                href: "/master/docs/slint/guide/language/types/#structs",
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

export const linkMap = {
    AnimationRef: {
        href: "/reference/builtins/animations",
    },
    BorderRadiusRectangle: {
        href: "/reference/elements/rectangle#border-radius-properties",
    },
    ColorsRef: {
        href: "/reference/builtins/colors",
    },
    CommonProperties: {
        href: "/reference/overview",
    },
    DebugFn: {
        href: "/reference/builtins/builtinfunctions#debug",
    },
    FocusHandling: {
        href: "/guide/focus",
    },
    GridLayout: {
        href: "/reference/layouts/gridlayout",
    },
    HorizontalBox: {
        href: "/reference/std-widgets/horizontalbox",
    },
    HorizontalLayout: {
        href: "/reference/layouts/horizontallayout",
    },
    Image: {
        href: "/reference/elements/image",
    },
    ListView: {
        href: "/reference/std-widgets/listview",
    },
    LineEdit: {
        href: "/reference/std-widgets/lineedit",
    },
    Path: {
        href: "/reference/elements/path",
    },
    ProgressIndicator: {
        href: "/reference/std-widgets/progressindicator",
    },
    Rectangle: {
        href: "/reference/elements/rectangle",
    },
    ScrollView: {
        href: "/reference/std-widgets/scrollview",
    },
    StandardButton: {
        href: "/reference/std-widgets/standardbutton",
    },
    StyleWidgets: {
        href: "/reference/std-widgets/style",
    },
    Text: {
        href: "/reference/elements/text/",
    },
    TextInput: {
        href: "/reference/elements/textinput/",
    },
    Timer: {
        href: "/reference/elements/timer/",
    },
    VerticalBox: {
        href: "/reference/std-widgets/verticalbox",
    },
    VerticalLayout: {
        href: "/reference/layouts/verticallayout",
    },
} as const;
