// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

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
        source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
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

        if (error_string != "") {
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
            let instance = component.create(canvas_id);
            instance.show();

            if (script) {
                let api_instance = create_instance(component, instance);

                let f = eval(cleanup_script(script));
                f(api_instance);
            }

            slint.run_event_loop();
        }
    }

    const params = new URLSearchParams(window.location.search);
    const code = params.get("snippet");
    const load_url = params.get("load_url");
    const style = params.get("style") || "";

    let script = params.get("script");

    if (code) {
        main_source = code;
    } else if (load_url) {
        base_url = load_url;
        const response = fetch(load_url).then(r => r.text());
        const script_url = params.get("script_url");
        if (script_url) {
            script = await fetch(script_url).then(r => r.text());
        }
        main_source = await response;
    }
    update_preview();
})();


function cleanup_script(script: string): string {
    let match = script.match(/let\s+(\w+)\s*=\s*require\(\s*["'][^"'\n]*\.slint["']/);
    script = script.replace(/^#![^\n]*\n/, "");
    if (match && match[1]) {
        let re = new RegExp("let\\s+(\\w+)\\s*=\\s*new\\s*" + match[1] + "\\s*\\.\\s*\\w+\\(\\);?");
        let m = script.match(re);
        if (m && m[1]) {
            script = script.replace(re, "");
            script = "let " + m[1] + " = slint;\n" + script;
        }
    }
    script = script.replace(/let\s+(\w+)\s*=\s*require\([^)\n]*\)[;\n]/g, "");
    return "(function(slint) { " + script + " })";
}


// FIXME: this should be in slint itself
function create_instance(c: slint.WrappedCompiledComp, comp: slint.WrappedInstance): any {
    class Component {
        protected comp: any;

        constructor(comp: any) {
            this.comp = comp;
        }

        run() {
            this.show();
        }

        show() {
            this.comp.show();
        }

        hide() {
            this.comp.hide()
        }
    }

    interface Callback {
        (): any;
        setHandler(cb: any): void;
    }

    let ret = new Component(comp);
    c.properties().forEach((x: string) => {
        Object.defineProperty(ret, x.replace(/-/g, '_'), {
            get() { return comp.get_property(x); },
            set(newValue) { comp.set_property(x, newValue); },
            enumerable: true,
        })
    });
    c.callbacks().forEach((x: string) => {
        Object.defineProperty(ret, x.replace(/-/g, '_'), {
            get() {
                let callback = function () { return comp.invoke_callback(x, [...arguments]); } as Callback;
                callback.setHandler = function (callback) { comp.set_callback(x, callback) };
                return callback;
            },
            enumerable: true,
        })
    });
    return ret;
}
