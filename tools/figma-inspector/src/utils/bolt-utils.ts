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
    callback: (data: EventTS[Key]) => any,
    listenOnce = false,
) => {
    const func = (event: MessageEvent<any>) => {
        if (event.data.pluginMessage.event === eventName) {
            callback(event.data.pluginMessage.data);
            if (listenOnce) {
                window.removeEventListener("message", func); // Remove Listener so we only listen once
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
