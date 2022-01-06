// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

import sixtyfps_init, * as sixtyfps from "../../../wasm-interpreter/sixtyfps_wasm_interpreter.js";

(async function () {
    await sixtyfps_init();

    var base_url = "";

    /// Index by url. Inline documents will use the empty string.
    var loaded_documents: Map<string, string> = new Map;

    let main_source = `
import { SpinBox, Button, CheckBox, Slider, GroupBox } from "sixtyfps_widgets.60";
export Demo := Window {
    width:  300px;   // Width in logical pixels. All 'px' units are automatically scaled with screen resolution.
    height: 300px;
    t:= Text {
        text: "Hello World";
        font-size: 24px;
    }
    Image {
        y: 50px;
        source: @image-url("https://sixtyfps.io/resources/logo_scaled.png");
    }
}
`

    function update_preview() {
        let div = document.getElementById("preview") as HTMLDivElement;
        setTimeout(function () { render_or_error(main_source, base_url, div); }, 1);
    }

    async function render_or_error(source: string, base_url: string, div: HTMLDivElement) {
        let canvas_id = 'canvas_' + Math.random().toString(36).substr(2, 9);
        let canvas = document.createElement("canvas");
        canvas.width = 800;
        canvas.height = 600;
        canvas.id = canvas_id;
        div.innerHTML = "";
        div.appendChild(canvas);

        let { component, error_string } = await sixtyfps.compile_from_string_with_style(source, base_url, style, async (url: string): Promise<string> => {
            let file_source = loaded_documents.get(url);
            if (file_source === undefined) {
                const response = await fetch(url);
                let doc = await response.text();
                loaded_documents.set(url, doc);
                return doc;
            }
            return file_source;
        });

        if (error_string != "") {
            let text = document.createTextNode(error_string);
            let p = document.createElement('pre');
            p.appendChild(text);
            div.innerHTML = "<pre style='color: red; background-color:#fee; margin:0'>" + p.innerHTML + "</pre>";
        } else {
            document.getElementById("spinner").remove()
        }

        if (component !== undefined) {
            component.run(canvas_id)
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
