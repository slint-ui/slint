// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/** biome-ignore-all lint/nursery/noFloatingPromises: top-level async IIFE drives preview lifecycle */

import slint_init, * as slint from "@interpreter/slint_wasm_interpreter.js";
import { edition_from_url } from "./edition";
import { is_project_url, load_project } from "./project";

(async function () {
    await slint_init();

    let base_url = "";
    let project_url: string | undefined;

    /// Index by url. Inline documents will use the empty string.
    const loaded_documents: Map<string, string> = new Map();

    let main_source = `
import { SpinBox, Button, CheckBox, Slider, GroupBox } from "std-widgets.slint";
export component Demo inherits Window {
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

    function show_error(error: string, div: HTMLDivElement) {
        const text = document.createTextNode(error);
        const p = document.createElement("pre");
        p.appendChild(text);
        div.innerHTML =
            "<pre style='color: red; background-color:#fee; margin:0'>" +
            p.innerHTML +
            "</pre>";
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
                async (url: string): Promise<string | null> => {
                    const file_source = loaded_documents.get(url);
                    if (file_source === undefined) {
                        const response = await fetch(url);
                        if (!response.ok) {
                            return null;
                        }
                        const doc = await response.text();
                        loaded_documents.set(url, doc);
                        return doc;
                    }
                    return file_source;
                },
                undefined,
                edition,
                project_url,
            );

        if (error_string !== "") {
            show_error(error_string, div);
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
    const edition = edition_from_url(params);

    if (code) {
        main_source = code;
    } else if (load_url) {
        let url = new URL(load_url, window.location.href);
        try {
            if (is_project_url(url)) {
                const project = await load_project(url);
                project_url = url.href;
                loaded_documents.set(project_url, project.source);
                url = project.main;
            }
            base_url = url.href;
            const response = await fetch(base_url);
            main_source = await response.text();
        } catch (e) {
            show_error(
                String(e),
                document.getElementById("preview") as HTMLDivElement,
            );
            return;
        }
    }
    update_preview();
})();
