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
    // The callback likely receives the whole message payload now
    callback: (data: EventTS[Key] & { type: Key }) => any, // Adjust type if needed
    listenOnce = false,
) => {
    const func = (pluginMessage: any) => {
        // The message from figma.ui.on is the payload directly
        console.log(`[Backend Listener Raw Msg]:`, pluginMessage); // <-- Add Raw Log
    const func = (pluginMessage: any) => {
        // The message from figma.ui.on is the payload directly
        // console.log(`[Backend Listener Raw Msg]:`, pluginMessage); // <-- Add Raw Log

        // --- Check if the received message has the correct type ---
        if (pluginMessage && pluginMessage.type === eventName) {
            // console.log(`[Backend Listener Matched Type]: ${eventName}`); // <-- Add Match Log
            callback(pluginMessage); // Pass the received payload
            if (listenOnce) {
                figma.ui.off("message", func);
            }
        }
    };

    console.log(`[Backend] Attaching listener for type: ${eventName}`); // <-- Add Attach Log
    figma.ui.on("message", func); // Use figma.ui.on directly
};

export const getStore = async (key: string) => {
    const value = await figma.clientStorage.getAsync(key);
    return value;
};

export const setStore = async (key: string, value: string) => {
    await figma.clientStorage.setAsync(key, value);
};

