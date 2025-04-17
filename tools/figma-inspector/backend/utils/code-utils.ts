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

export async function updateUI() {
    console.log("[updateUI] Function execution started.");
    const selection = figma.currentPage.selection;
    let title = "No selection";
    let slintSnippet: string | null = null;
    let messagePayload: any = null; // Define outside try block

    try { // --- Wrap more logic ---
        if (selection.length === 1) {
            const node = selection[0];
            title = node.name;
            // Keep inner try...catch for specific snippet generation error
            try {
                console.log(`[updateUI] Calling generateSlintSnippet for node: ${node.name}`);
                slintSnippet = await generateSlintSnippet(node);
                // --- Log immediately after await ---
                console.log(`[updateUI] generateSlintSnippet returned: ${slintSnippet ? 'Snippet received' : 'null'}`);
            } catch (snippetError) {
                console.error("[updateUI] Caught error DURING generateSlintSnippet:", snippetError);
                slintSnippet = "// Error generating snippet. See console.";
            }
        } else if (selection.length > 1) {
            title = "Multiple items selected";
        }

        // --- Create payload and log within the try block ---
        messagePayload = {
            type: "updatePropertiesCallback",
            title: title,
            slintSnippet: slintSnippet ?? "// Could not generate snippet.",
        };

        console.log(`[updateUI] Preparing to post message. Snippet is null: ${slintSnippet === null}`);
        console.log(`[updateUI] Payload:`, JSON.stringify(messagePayload));
        // --- End create payload and log ---

    } catch (outerError) { // --- Catch errors during selection handling or payload creation ---
        console.error("[updateUI] >>> ERROR before posting message:", outerError);
        // Attempt to create a fallback error payload
        messagePayload = {
            type: "updatePropertiesCallback",
            title: "Error",
            slintSnippet: `// Error preparing UI update: ${outerError instanceof Error ? outerError.message : outerError}`
        };
         console.log(`[updateUI] Created fallback error payload.`);
    }

    // --- Post Message (outside the main try block, but payload is guaranteed to exist) ---
    if (messagePayload) {
        try {
            figma.ui.postMessage(messagePayload);
            console.log(`[updateUI] Successfully posted message to UI.`);
        } catch (postError) {
             console.error(`[updateUI] Error POSTING message to UI:`, postError);
        }
    } else {
        console.error("[updateUI] messagePayload was unexpectedly null, cannot post to UI.");
    }
}