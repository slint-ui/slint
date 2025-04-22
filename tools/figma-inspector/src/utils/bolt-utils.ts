// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT

import { manifest } from "../../figma.config";
import type { Message, PluginMessageEvent } from "../globals";
import type { EventTS } from "../../shared/universals";

export const dispatch = (msg: Message, global = false, origin = "*") => {
    const data: PluginMessageEvent = { pluginMessage: msg };
    if (!global) {
        data.pluginId = manifest.id;
    }
    parent.postMessage(data, origin);
};
/*
// Define the types for the events that can be dispatched
export type EventTS = {
    exportToFiles: any;
    monitorVariableChanges: any;
    checkVariableChanges: any;
    copyToClipboard: any;
    generateSnippetRequest: any;
    // Add more events as needed
};
*/

export const dispatchTS = <Key extends keyof EventTS>(
    event: Key,
    data: EventTS[Key],
    global = false,
    origin = "*",
) => {
    dispatch({ event, ...data }, global, origin);
};

export const listenTS = <Key extends keyof EventTS>(
    eventName: Key,
    // --- Adjust callback type to expect the whole message ---
    callback: (data: any) => any, // Use 'any' for simplicity or define a more specific type
    listenOnce = false,
) => {
    const func = (event: MessageEvent<any>) => {
        // --- Check for pluginMessage existence ---
        if (event.data && event.data.pluginMessage) {
            const pluginMessage = event.data.pluginMessage;

            // --- Check for 'type' property instead of 'event' ---
            if (pluginMessage.type === eventName) {
                // --- Pass the whole pluginMessage object to the callback ---
                callback(pluginMessage);
                if (listenOnce) {
                    window.removeEventListener("message", func);
                }
            }
        }
    };
    window.addEventListener("message", func);
};

export const getColorTheme = () => {
    if (window?.matchMedia) {
        if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
            return "dark";
        }
        if (window.matchMedia("(prefers-color-scheme: light)").matches) {
            return "light";
        }
    }
    return "light";
};

export const subscribeColorTheme = (
    callback: (mode: "light" | "dark") => void,
) => {
    if (window?.matchMedia) {
        window
            .matchMedia("(prefers-color-scheme: dark)")
            .addEventListener("change", ({ matches }) => {
                if (matches) {
                    console.log("change to dark mode!");
                    callback("dark");
                } else {
                    console.log("change to light mode!");
                    callback("light");
                }
            });
    }
};
