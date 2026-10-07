// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Allocate names in stable caller order; only collisions receive a suffix. */
export function nameAllocator(
    reserved: readonly string[] = [],
    separator = "-",
) {
    const used = new Set(reserved);
    return (base: string): string => {
        let name = base;
        for (let suffix = 2; used.has(name); suffix++)
            name = `${base}${separator}${suffix}`;
        used.add(name);
        return name;
    };
}

export function identifier(name: string): string {
    const clean =
        name
            .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
            .replace(/[^a-zA-Z0-9]+/g, "-")
            .replace(/^-|-$/g, "")
            .toLowerCase() || "unnamed";
    // A prefix also keeps numeric Figma option names valid Slint identifiers.
    return /^[a-z]/.test(clean) ? clean : `option-${clean}`;
}

export function typeName(name: string): string {
    if (/^[A-Z][a-zA-Z0-9]*$/.test(name)) return name;
    return identifier(name)
        .split("-")
        .map((word) => word[0].toUpperCase() + word.slice(1))
        .join("");
}

// Slint keywords cannot be emitted bare as enum members.
export const enumReservedNames = [
    "true",
    "false",
    "if",
    "else",
    "for",
    "in",
    "out",
    "in-out",
    "property",
    "callback",
    "component",
    "export",
    "import",
    "inherits",
    "struct",
    "enum",
    "global",
    "animate",
    "states",
    "transitions",
    "return",
    "private",
    "public",
];

// Names emitted by the visual converter or provided by Slint itself.
export const reservedTypeNames = [
    "Demo",
    "Rectangle",
    "Text",
    "Image",
    "Path",
    "TouchArea",
    "FocusScope",
    "Window",
    "TextInput",
    "Flickable",
    "Clip",
    "Opacity",
    "Layer",
    "Row",
    "GridLayout",
    "HorizontalLayout",
    "VerticalLayout",
    "FlexboxLayout",
    "BoxShadow",
    "ComponentContainer",
    "Transform",
    "ClippedImage",
    "ContextMenuArea",
    "WindowMoveArea",
    "SwipeGestureHandler",
    "ScaleRotateGestureHandler",
    "DragArea",
    "DropArea",
    "Menu",
    "MenuItem",
    "MenuSeparator",
    "MenuBar",
    "ContextMenu",
    "Timer",
    "PopupWindow",
];
