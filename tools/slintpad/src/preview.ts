// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/** biome-ignore-all lint/nursery/noFloatingPromises: <explanation> */

import slint_init, * as slint from "@interpreter/slint_wasm_interpreter.js";

(async function () {
    await slint_init();

    let base_url = "";

    /// Index by url. Inline documents will use the empty string.
    const loaded_documents: Map<string, string> = new Map();

    let main_source = `
import { SpinBox, Button, CheckBox, Slider, GroupBox } from "std-widgets.slint";
export Demo := Window {
    width:  300px;   // Width in logical pixels. All 'px' units are automatically scaled with screen resolution.
    height: 300px;
    t:= Text {
        text: "Hello World";
        font-size: 24px;
    }
    Image {
        y: 50px;
        source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
    }
}
`;

    function update_preview() {
        const div = document.getElementById("preview") as HTMLDivElement;
        setTimeout(function () {
            render_or_error(main_source, base_url, div);
        }, 1);
    }

    async function render_or_error(
        source: string,
        base_url: string,
        div: HTMLDivElement,
    ) {
        const canvas_id = "canvas_" + Math.random().toString(36).slice(2, 11);
        const canvas = document.createElement("canvas");
        canvas.width = 800;
        canvas.height = 600;
        canvas.id = canvas_id;
        div.innerHTML = "";
        div.appendChild(canvas);

        const { component, error_string } =
            await slint.compile_from_string_with_style(
                source,
                base_url,
                style,
                async (url: string): Promise<string> => {
                    const file_source = loaded_documents.get(url);
                    if (file_source === undefined) {
                        const response = await fetch(url);
                        const doc = await response.text();
                        loaded_documents.set(url, doc);
                        return doc;
                    }
                    return file_source;
                },
            );

        if (error_string !== "") {
            const text = document.createTextNode(error_string);
            const p = document.createElement("pre");
            p.appendChild(text);
            div.innerHTML =
                "<pre style='color: red; background-color:#fee; margin:0'>" +
                p.innerHTML +
                "</pre>";
        } else {
            const spinner = document.getElementById("spinner");
            if (spinner !== null) {
                spinner.remove();
            }
        }

        if (component !== undefined) {
            component.run(canvas_id);
        }
    }

    const params = new URLSearchParams(window.location.search);
    const code = params.get("snippet");
    const load_url = params.get("load_url");
    const style = params.get("style") || "";

    if (code) {
        main_source = code;
    } else if (load_url) {
        base_url = load_url;
        const response = await fetch(load_url);
        main_source = await response.text();
    }
    update_preview();
})();
