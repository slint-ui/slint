// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { EventTS } from "../../shared/universals";

function dispatch(data: any, origin = "*") {
    figma.ui.postMessage(data, {
        origin,
    });
}

export function dispatchTS<Key extends keyof EventTS>(
    event: Key,
    data: EventTS[Key],
    origin = "*",
) {
    dispatch({ type: event, ...data }, origin);
}

export function listenTS<Key extends keyof EventTS>(
    eventName: Key,
    callback: (data: EventTS[Key] & { type: Key }) => any,
    listenOnce = false,
) {
    const func = (pluginMessage: any) => {
        if (pluginMessage && pluginMessage.type === eventName) {
            callback(pluginMessage);
            if (listenOnce) {
                figma.ui.off("message", func);
            }
        }
    };

    figma.ui.on("message", func);
}
