// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
    DefinitionPosition,
    PropertiesView,
} from "../../../tools/online_editor/src/shared/properties";

let node = PropertiesView.createNode();
let view = new PropertiesView(node);
document.body.appendChild(node);

const vscode = acquireVsCodeApi();
view.property_clicked = (uri, p) => {
    vscode.postMessage({ command: "property_clicked", uri: uri, property: p });
};
view.change_property = (uri, p, new_value, old_value) => {
    vscode.postMessage({
        command: "change_property",
        uri: uri,
        property: p,
        old_value: old_value,
        new_value: new_value,
    });
};

window.addEventListener("message", async (event) => {
    if (event.data.command === "set_properties") {
        view.set_properties(event.data.properties);
    } else if (event.data.command === "clear") {
        view.set_properties({
            element: null,
            properties: [],
            source_uri: "",
            source_version: 0,
        });
    }
});
