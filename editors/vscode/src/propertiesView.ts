// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore codicon

import {
    PropertiesView,
    SetBindingResponse,
} from "../../../tools/slintpad/src/shared/properties";

const vscode = acquireVsCodeApi();

let set_binding_start = Date.now();
const set_binding_response_timeout = 5 * 1000;
let set_binding_response: SetBindingResponse | null = null;

let node = PropertiesView.createNode();
let view = new PropertiesView(
    node,
    (doc, element, property_name, new_value, dry_run) => {
        vscode.postMessage({
            command: "change_property",
            document: doc,
            element_range: element,
            property_name: property_name,
            new_value: new_value,
            dry_run: dry_run,
        });
        return ensure_set_binding_response(set_binding_response_timeout);
    },
    "codicon codicon-trash",
    (doc, element, property_name) => {
        vscode.postMessage({
            command: "remove_binding",
            document: doc,
            element_range: element,
            property_name: property_name,
        });
        return Promise.resolve(true); // Cheat: Claim it was a success...
    },
    "codicon codicon-add",
);

async function ensure_set_binding_response(
    timeout: number,
): Promise<SetBindingResponse> {
    set_binding_start = Date.now();
    return new Promise(wait_for_set_binding_response);

    function wait_for_set_binding_response(resolve, reject) {
        if (set_binding_response !== null) {
            const r = set_binding_response;
            set_binding_response = null;
            resolve(r);
        }
        if (timeout && Date.now() - set_binding_start >= timeout) {
            reject(new Error("Timeout waiting for result of set_binding call"));
        } else {
            setTimeout(
                wait_for_set_binding_response.bind(this, resolve, reject),
                100,
            );
        }
    }
}

document.body.appendChild(node);

window.addEventListener("message", async (event) => {
    if (event.data.command === "set_properties") {
        view.set_properties(event.data.properties);
    } else if (event.data.command === "show_welcome") {
        view.show_welcome(event.data.message ?? "Something went wrong");
    } else if (event.data.command === "set_binding_response") {
        set_binding_response = event.data.response;
    }
});
