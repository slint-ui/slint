// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Message, PluginMessageEvent } from "../../src/globals";
import type { EventTS } from "../../shared/universals";

export const dispatch = (data: any, origin = "*") => {
    figma.ui.postMessage(data, {
        origin,
    });
};

export const dispatchTS = <Key extends keyof EventTS>(
    event: Key,
    data: EventTS[Key],
    origin = "*",
) => {
    dispatch({ event, data }, origin);
};

export const listenTS = <Key extends keyof EventTS>(
    eventName: Key,
    callback: (data: EventTS[Key]) => any,
    listenOnce = false,
) => {
    const func = (event: any) => {
        if (event.event === eventName) {
            callback(event);
            if (listenOnce) {
                figma.ui?.off("message", func); // Remove Listener so we only listen once
            }
        }
    };

    figma.ui.on("message", func);
};

export const getStore = async (key: string) => {
    const value = await figma.clientStorage.getAsync(key);
    return value;
};

export const setStore = async (key: string, value: string) => {
    await figma.clientStorage.setAsync(key, value);
};

export function getStatus(selectionCount: number) {
    if (selectionCount === 0) {
        return "Please select a layer";
    }
    if (selectionCount > 1) {
        return "Please select only one layer";
    }
    return "Slint properties:";
}

const itemsToKeep = [
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "width",
    "height",
    "fill",
    "opacity",
    "border-radius",
    "fill",
    "stroke-width",
    "stroke",
];

type StyleObject = {
    [key: string]: string;
};

function transformStyle(styleObj: StyleObject): string {
    const filteredEntries = Object.entries(styleObj)
        .filter(([key]) => itemsToKeep.includes(key))
        .map(([key, value]) => {
            let finalKey = key;
            let finalValue = value;

            switch (key) {
                case "fill":
                    finalKey = "background";
                    break;
                case "stroke":
                    finalKey = "border-color";
                    break;
                case "stroke-width":
                    finalKey = "border-width";
                    break;
                case "font-family":
                    finalValue = `"${value}"`;
                    break;
            }

            if (value.includes("linear-gradient")) {
                return `  ${finalKey}: @${finalValue}`;
            }

            return `  ${finalKey}: ${finalValue}`;
        });

    return filteredEntries.length > 0 ? `${filteredEntries.join(";\n")};` : "";
}

export async function updateUI() {
    const title = getStatus(figma.currentPage.selection.length);
    let slintProperties = "";

    if (figma.currentPage.selection.length === 1) {
        const cssProperties =
            await figma.currentPage.selection[0].getCSSAsync();
        slintProperties = transformStyle(cssProperties);
    }

    dispatchTS("updatePropertiesCallback", { title, slintProperties });
}

export async function getSlintSnippet(): Promise<string> {
    const cssProperties = await figma.currentPage.selection[0].getCSSAsync();
    const slintProperties = transformStyle(cssProperties);

    let elementName = "Rectangle";
    const node = figma.currentPage.selection[0].type;
    if (node === "TEXT") {
        elementName = "Text";
    }

    return `${elementName} {\n${slintProperties}\n}`;
}
