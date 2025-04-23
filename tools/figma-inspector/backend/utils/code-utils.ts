// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Message, PluginMessageEvent } from "../../src/globals";
import type { EventTS } from "../../shared/universals";
import { generateSlintSnippet } from "./property-parsing.js";

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
    dispatch({ type: event, ...data }, origin);
};

export const listenTS = <Key extends keyof EventTS>(
    eventName: Key,
    callback: (data: EventTS[Key] & { type: Key }) => any,
    listenOnce = false,
) => {
    // --- Define func only ONCE ---
    const func = (pluginMessage: any) => {
        // The message from figma.ui.on is the payload directly
        console.log(`[Backend Listener Raw Msg]:`, pluginMessage); // <-- Uncomment if you want this log

        // --- Check if the received message has the correct type ---
        if (pluginMessage && pluginMessage.type === eventName) {
            console.log(`[Backend Listener Matched Type]: ${eventName}`); // <-- Uncomment if you want this log
            callback(pluginMessage); // Pass the received payload
            if (listenOnce) {
                figma.ui.off("message", func);
            }
        }
    };
    // --- End single definition ---

    console.log(`[Backend] Attaching listener for type: ${eventName}`);
    figma.ui.on("message", func);
};
export const getStore = async (key: string) => {
    const value = await figma.clientStorage.getAsync(key);
    return value;
};

export const setStore = async (key: string, value: string) => {
    await figma.clientStorage.setAsync(key, value);
};
