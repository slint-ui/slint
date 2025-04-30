// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT

import { manifest } from "../../figma.config";
import type { Message, PluginMessageEvent } from "../globals";
import type { EventTS } from "../../shared/universals";

export function dispatch(msg: Message, global = false, origin = "*") {
    const data: PluginMessageEvent = { pluginMessage: msg };
    if (!global) {
        data.pluginId = manifest.id;
    }
    parent.postMessage(data, origin);
}

export function dispatchTS<Key extends keyof EventTS>(
    event: Key, // Parameter name is 'event'
    data: EventTS[Key],
    global = false,
    origin = "*",
) {
    dispatch({ type: event, ...data }, global, origin);
}

export function listenTS<Key extends keyof EventTS>(
    eventName: Key,
    callback: (data: EventTS[Key]) => any,
    listenOnce = false,
) {
    const func = (event: MessageEvent<any>) => {
        // --- Check for pluginMessage existence ---
        if (event.data && event.data.pluginMessage) {
            const pluginMessage = event.data.pluginMessage;

            if (pluginMessage.type === eventName) {
                // We've verified the type, so we can safely cast
                const eventData = pluginMessage as EventTS[Key];
                callback(eventData);
                if (listenOnce) {
                    window.removeEventListener("message", func);
                }
            }
        }
    };
    window.addEventListener("message", func);
}

export function getColorTheme(): "light" | "dark" {
    if (window?.matchMedia) {
        if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
            return "dark";
        }
        if (window.matchMedia("(prefers-color-scheme: light)").matches) {
            return "light";
        }
    }
    return "light";
}

export function subscribeColorTheme(
    callback: (mode: "light" | "dark") => void,
) {
    if (window?.matchMedia) {
        window
            .matchMedia("(prefers-color-scheme: dark)")
            .addEventListener("change", ({ matches }) => {
                if (matches) {
                    callback("dark");
                } else {
                    callback("light");
                }
            });
    }
}
