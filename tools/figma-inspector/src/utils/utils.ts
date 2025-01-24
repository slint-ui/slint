// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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

export function writeTextToClipboard(str: string) {
    const prevActive = document.activeElement;
    const textArea = document.createElement("textarea");

    textArea.value = str;

    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";

    document.body.appendChild(textArea);

    textArea.focus();
    textArea.select();

    return new Promise<void>((res, rej) => {
        document.execCommand("copy") ? res() : rej();
        textArea.remove();

        if (prevActive && prevActive instanceof HTMLElement) {
            prevActive.focus();
        }
    });
}

export function copyToClipboard(slintProperties: string) {
    writeTextToClipboard(slintProperties);
    dispatchTS("copyToClipboard", {
        result: true,
    });
}
